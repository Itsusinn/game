//! Main-thread-only bridge. Simulation and transport remain ordinary Rust.
use client::{mesh::Mesh, movement::Movement, presentation, session::Session};
use godot::{classes::Node, prelude::*};
use protocol::PlayerInput;

struct GameExtension;
#[gdextension]
unsafe impl ExtensionLibrary for GameExtension {}

#[derive(GodotClass)]
#[class(base=Node, init)]
struct GameBridge {
    base: Base<Node>,
    session: Session,
    movement: Movement,
    attack: bool,
    pickup: bool,
}

#[godot_api]
impl GameBridge {
    #[func]
    fn set_identity_path(&mut self, path: GString) {
        if self.session.transport.is_none() && !self.session.local_running() {
            self.session.identity_path = path.to_string().into();
        }
    }
    #[func]
    fn start_singleplayer(&mut self, path: GString, name: GString) {
        self.session.name = name.to_string();
        self.session.start_singleplayer(path.to_string().into());
        self.reset_motion();
    }

    #[func]
    fn start_demo(&mut self) {
        self.session.load_demo();
        self.reset_motion();
    }

    #[func]
    fn connect_game(&mut self, address: GString, name: GString, certificate: GString) {
        self.session.address = address.to_string();
        self.session.name = name.to_string();
        self.session.certificate = certificate.to_string();
        self.session.connect();
        self.reset_motion();
    }

    #[func]
    fn disconnect_game(&mut self) {
        // Drop signals cancellation without joining a network worker on Godot's thread.
        self.session.disconnect();
        self.reset_motion();
    }

    #[func]
    fn clear_actions(&mut self) {
        self.attack = false;
        self.pickup = false;
    }

    #[func]
    fn send_chat(&mut self, text: GString) {
        self.session.chat(text.to_string());
    }

    #[func]
    fn edit_block(&mut self, yaw: f32, pitch: f32, place: bool) {
        self.session.edit_block(yaw, pitch, place);
    }

    #[func]
    fn block_target(&self, yaw: f32, pitch: f32) -> VarDictionary {
        let m = &self.session.model;
        if !m.ready || self.session.demo {
            return VarDictionary::new();
        }
        self.session
            .block_target(yaw, pitch)
            .map_or_else(VarDictionary::new, |p| {
                vdict! { "position" => Vector3i::new(p[0],p[1],p[2]),
                "name" => protocol::block_item_name(m.terrain.get(p)).unwrap_or("") }
            })
    }

    #[func]
    fn take_dirty_chunks(&mut self) -> Array<Vector3i> {
        std::mem::take(&mut self.session.model.dirty_chunks)
            .into_iter()
            .map(|p| Vector3i::new(p[0], p[1], p[2]))
            .collect()
    }

    #[func]
    fn container_target(&self, yaw: f32, pitch: f32) -> VarDictionary {
        let m = &self.session.model;
        if self.session.demo || !m.ready {
            return VarDictionary::new();
        }
        self.session.container_target(yaw, pitch).map_or_else(
            VarDictionary::new,
            |p| vdict! { "position" => Vector3i::new(p[0],p[1],p[2]) },
        )
    }

    #[func]
    fn open_container(&mut self, position: Vector3i) {
        self.session.container(protocol::ContainerCommand::Open {
            position: [position.x, position.y, position.z],
        });
    }

    #[func]
    fn close_container(&mut self) {
        self.session.container(protocol::ContainerCommand::Close);
    }

    #[func]
    fn transfer_container(&mut self, request: VarDictionary) {
        let get = |key: &str| request.get(key).and_then(|v| v.try_to::<i64>().ok());
        let Some(position) = request
            .get("position")
            .and_then(|v| v.try_to::<Vector3i>().ok())
        else {
            return;
        };
        let (Some(revision), Some(inventory_revision), Some(slot), Some(quantity)) = (
            get("revision"),
            get("inventory_revision"),
            get("slot"),
            get("quantity"),
        ) else {
            return;
        };
        let (Ok(revision), Ok(inventory_revision), Ok(slot), Ok(quantity)) = (
            u64::try_from(revision),
            u64::try_from(inventory_revision),
            u8::try_from(slot),
            u32::try_from(quantity),
        ) else {
            return;
        };
        let target = match get("target") {
            Some(-1) | None => None,
            Some(i) => {
                let Ok(i) = u8::try_from(i) else {
                    return;
                };
                Some(i)
            }
        };
        let Some(deposit) = request.get("deposit").and_then(|v| v.try_to::<bool>().ok()) else {
            return;
        };
        self.session
            .container(protocol::ContainerCommand::Transfer {
                position: [position.x, position.y, position.z],
                revision,
                inventory_revision,
                slot,
                target,
                quantity,
                deposit,
            });
    }

    #[func]
    fn select_hotbar(&mut self, slot: i64) {
        if let Ok(slot) = u8::try_from(slot) {
            self.session
                .inventory(protocol::InventoryCommand::Select { slot });
        }
    }

    #[func]
    fn move_inventory(&mut self, revision: i64, from: i64, to: i64, quantity: i64) {
        if let (Ok(revision), Ok(from), Ok(to), Ok(quantity)) = (
            u64::try_from(revision),
            u8::try_from(from),
            u8::try_from(to),
            u32::try_from(quantity),
        ) {
            self.session.inventory(protocol::InventoryCommand::Move {
                revision,
                from,
                to,
                quantity,
            });
        }
    }

    #[func]
    fn quick_move_inventory(&mut self, revision: i64, slot: i64) {
        if let (Ok(revision), Ok(slot)) = (u64::try_from(revision), u8::try_from(slot)) {
            self.session
                .inventory(protocol::InventoryCommand::QuickMove { revision, slot });
        }
    }

    #[func]
    fn advance(&mut self, delta: f64, controls: VarDictionary) -> VarDictionary {
        let was_ready = self.session.model.ready;
        self.session.tick();
        if !was_ready && self.session.model.ready {
            // Give the host one frame to adopt the restored camera before it
            // submits the first input of the new connection.
            self.movement.sync(&self.session);
            return self.snapshot();
        }
        let number = |key: &str| {
            controls
                .get(key)
                .and_then(|v| v.try_to::<f64>().ok())
                .unwrap_or(0.0) as f32
        };
        let flag = |key: &str| {
            controls
                .get(key)
                .and_then(|v| v.try_to::<bool>().ok())
                .unwrap_or(false)
        };
        let mut input = PlayerInput {
            forward: number("forward"),
            strafe: number("strafe"),
            yaw: number("yaw"),
            pitch: number("pitch"),
            jump: flag("jump"),
            attack: false,
            pickup: false,
        };
        if !input.valid() || !delta.is_finite() || delta < 0.0 {
            self.session.status = "Invalid frame input".into();
            return self.snapshot();
        }
        self.attack |= flag("attack");
        self.pickup |= flag("pickup");
        let attack = &mut self.attack;
        let pickup = &mut self.pickup;
        self.movement.advance(&mut self.session, delta as f32, || {
            input.attack = std::mem::take(attack);
            input.pickup = std::mem::take(pickup);
            input
        });
        self.snapshot()
    }

    #[func]
    fn snapshot(&self) -> VarDictionary {
        let m = &self.session.model;
        let actors: VarArray = m
            .actors
            .iter()
            .filter(|a| a.id != m.player_id)
            .map(|a| {
                vdict! { "id" => i64::from(a.id), "kind" => i64::from(a.kind),
                "position" => vector(a.position), "yaw" => a.yaw,
                "hp" => a.hp, "max_hp" => a.max_hp }
                .to_variant()
            })
            .collect();
        let items: VarArray = m.items.iter().map(|i| {
            vdict! { "id" => i64::from(i.id), "position" => vector(i.position), "name" => i.name.as_str() }.to_variant()
        }).collect();
        let inventory: VarArray = m
            .inventory
            .iter()
            .map(|i| {
                vdict! { "name" => i.name.as_str(), "quantity" => i64::from(i.quantity) }
                    .to_variant()
            })
            .collect();
        let journal: VarArray = m.log.iter().map(|e| e.text.to_variant()).collect();
        let inventory_slots: VarArray = m
            .inventory
            .slots
            .iter()
            .map(|slot| {
                slot.as_ref().map_or_else(
                    || VarDictionary::new().to_variant(),
                    |i| {
                        vdict! { "name" => i.name.as_str(), "quantity" => i64::from(i.quantity) }
                            .to_variant()
                    },
                )
            })
            .collect();
        let container = m.container.as_ref().map_or_else(VarDictionary::new, |c| {
            let items: VarArray = c.items.iter().map(|i| vdict! {
                "name" => i.name.as_str(), "quantity" => i64::from(i.quantity)
            }.to_variant()).collect();
            vdict! { "position" => Vector3i::new(c.position[0],c.position[1],c.position[2]),
                "revision" => c.revision as i64, "items" => &items, "capacity" => protocol::CONTAINER_SLOTS as i64 }
        });
        vdict! {
            "ready" => m.ready, "demo" => self.session.demo,
            "local_running" => self.session.local_running(), "local_error" => self.session.local_error,
            "shutdown_pending" => self.session.shutdown_pending(),
            "connected" => self.session.transport.is_some(), "status" => self.session.status.as_str(),
            "generation" => self.session.generation as i64, "revision" => m.revision as i64,
            "loaded_sections" => m.terrain.blocks.len() as i64,
            "position" => vector(self.movement.position()), "body_position" => vector(m.body.position),
            "grounded" => m.body.grounded, "hp" => m.hp, "player_id" => i64::from(m.player_id),
            "pending" => m.pending.len() as i64,
            "spawn_rotation" => Vector2::new(m.spawn_rotation[0], m.spawn_rotation[1]),
            "inventory_slots" => &inventory_slots, "inventory_revision" => m.inventory.revision as i64,
            "selected_slot" => i64::from(m.inventory.selected), "stack_limit" => protocol::ITEM_STACK_LIMIT,
            "container" => &container, "container_error" => m.container_error.as_str(),
            "actors" => &actors, "items" => &items, "inventory" => &inventory, "journal" => &journal
        }
    }

    #[func]
    fn chunk_geometry(&self, origin: Vector3i) -> VarDictionary {
        let origin = [origin.x, origin.y, origin.z];
        if !protocol::valid_voxel(origin)
            || origin.iter().any(|v| v.rem_euclid(16) != 0)
            || !self.session.model.terrain.blocks.contains_key(&origin)
        {
            return VarDictionary::new();
        }
        geometry(presentation::terrain_chunk(&self.session.model, origin))
    }

    #[func]
    fn actor_geometry(&self, kind: i32) -> VarDictionary {
        let model = game_model_for_actor(kind);
        geometry(presentation::actors(&model))
    }

    #[func]
    fn item_geometry(&self) -> VarDictionary {
        let mut model = client::session::Model::default();
        model.items.push(protocol::GroundItem3d {
            id: 1,
            position: [0.0; 3],
            name: String::new(),
        });
        geometry(presentation::actors(&model))
    }

    fn reset_motion(&mut self) {
        self.movement = Movement::default();
        self.movement.sync(&self.session);
        self.attack = false;
        self.pickup = false;
    }
}

fn vector(p: [f32; 3]) -> Vector3 {
    Vector3::new(p[0], p[1], p[2])
}

fn game_model_for_actor(kind: i32) -> client::session::Model {
    let mut m = client::session::Model::default();
    m.actors.push(protocol::Actor3d {
        id: 1,
        kind: u8::from(kind != 0),
        position: [0.0; 3],
        yaw: 0.0,
        hp: 100,
        max_hp: 100,
    });
    m
}

fn geometry(mesh: Mesh) -> VarDictionary {
    let mut vertices = PackedVector3Array::new();
    let mut normals = PackedVector3Array::new();
    let mut colors = PackedColorArray::new();
    let mut uv = PackedVector2Array::new();
    let mut patterns = PackedVector2Array::new();
    // Convert the CPU mesh's counter-clockwise faces to Godot's clockwise winding.
    for tri in mesh.vertices.as_chunks::<3>().0 {
        for i in [0, 2, 1] {
            let v = tri[i];
            vertices.push(vector(v.position));
            normals.push(vector(v.normal));
            colors.push(Color::from_rgb(v.color[0], v.color[1], v.color[2]));
            uv.push(Vector2::new(v.uv[0], v.uv[1]));
            patterns.push(Vector2::new(v.pattern as f32, 0.0));
        }
    }
    vdict! { "vertices" => &vertices, "normals" => &normals, "colors" => &colors, "uv" => &uv, "uv2" => &patterns }
}
