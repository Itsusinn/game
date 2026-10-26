extends Node3D

const HudScript = preload("res://src/hud.gd")
const MaterialShader = preload("res://materials/voxel.gdshader")
var bridge: GameBridge
var camera: Camera3D
var hud: CanvasLayer
var environment: Environment
var sun: DirectionalLight3D
var terrain_root: Node3D
var entities: Dictionary = {}
var item_nodes: Dictionary = {}
var actor_meshes: Array[ArrayMesh] = []
var item_mesh: ArrayMesh
var material: ShaderMaterial
var state: Dictionary = {}
var revision := Vector2i(-1, -1)
var yaw := 0.0
var pitch := -0.10
var attack := false
var pickup := false
var captured := false
var focused := true
var capture_path := ""
var capture_frames := 64
var ready_frames := 0
var capture_pending := false
var started := Time.get_ticks_msec()
var options: Dictionary = {}

func _ready() -> void:
	bridge = GameBridge.new()
	bridge.name = "GameBridge"
	add_child(bridge)
	_setup_input()
	_setup_world()
	hud = HudScript.new()
	add_child(hud)
	hud.connect_requested.connect(_connect_game)
	hud.demo_requested.connect(_demo)
	hud.disconnect_requested.connect(_disconnect_game)
	hud.chat_requested.connect(func(text: String): bridge.send_chat(text))
	hud.fov_changed.connect(func(value: float): camera.fov = value)
	hud.lighting_changed.connect(func(enabled: bool): environment.sdfgi_enabled = enabled)
	options = _parse_options(OS.get_cmdline_user_args())
	if options.has("server"): hud.address.text = options.server
	if options.has("name"): hud.player_name.text = options.name
	if options.has("cert"): hud.certificate.text = options.cert
	if options.has("fov"): hud.fov_slider.value = clampf(float(options.fov),55,100)
	if options.has("yaw"): yaw = deg_to_rad(float(options.yaw))
	if options.has("capture"): capture_path = options.capture
	if options.has("capture-frames"): capture_frames = clampi(int(options["capture-frames"]),1,4096)
	if options.get("lighting", "gi") == "direct": hud.gi_toggle.button_pressed = false
	if options.has("demo"): _demo()
	if options.has("connect"): _connect_game(hud.address.text,hud.player_name.text,hud.certificate.text)
	state = bridge.snapshot()
	hud.update_state(state, captured)
	get_window().focus_exited.connect(_focus_lost)
	get_window().focus_entered.connect(func(): focused = true)

func _parse_options(args: PackedStringArray) -> Dictionary:
	var result := {}
	var i := 0
	while i < args.size():
		var key := args[i].trim_prefix("--")
		if key in ["demo", "connect"]:
			result[key] = true
		elif key in ["server", "name", "cert", "fov", "yaw", "capture", "capture-frames", "lighting"]:
			i += 1
			if i < args.size(): result[key] = args[i]
		i += 1
	return result

func _setup_input() -> void:
	var mappings := {"forward": [KEY_W, KEY_UP], "back": [KEY_S, KEY_DOWN],
		"left": [KEY_A, KEY_LEFT], "right": [KEY_D, KEY_RIGHT], "jump": [KEY_SPACE]}
	for action in mappings:
		if not InputMap.has_action(action): InputMap.add_action(action)
		for key in mappings[action]:
			var event := InputEventKey.new()
			event.physical_keycode = key
			InputMap.action_add_event(action,event)

func _setup_world() -> void:
	material = ShaderMaterial.new()
	material.shader = MaterialShader
	terrain_root = Node3D.new()
	terrain_root.name = "TerrainChunks"
	add_child(terrain_root)
	camera = Camera3D.new()
	camera.name = "FirstPersonCamera"
	camera.near = 0.05
	camera.far = 150
	camera.fov = 72
	camera.position = Vector3(28.5,8.58,32.5)
	camera.current = true
	add_child(camera)
	sun = DirectionalLight3D.new()
	sun.name = "Sun"
	sun.rotation_degrees = Vector3(-48,-35,0)
	sun.light_color = Color(1.0,0.94,0.82)
	sun.light_energy = 0.75
	sun.light_angular_distance = 0.6
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = 100
	add_child(sun)
	environment = Environment.new()
	environment.background_mode = Environment.BG_SKY
	var sky := Sky.new()
	var sky_material := ProceduralSkyMaterial.new()
	sky_material.sky_top_color = Color("8aaac1")
	sky_material.sky_horizon_color = Color("c8d6d4")
	sky_material.ground_horizon_color = Color("c8d6d4")
	sky_material.ground_bottom_color = Color("52604c")
	sky.sky_material = sky_material
	environment.sky = sky
	environment.ambient_light_source = Environment.AMBIENT_SOURCE_SKY
	environment.ambient_light_energy = 0.45
	environment.tonemap_mode = Environment.TONE_MAPPER_FILMIC
	environment.sdfgi_enabled = true
	environment.sdfgi_min_cell_size = 0.35
	environment.sdfgi_cascades = 4
	environment.sdfgi_use_occlusion = true
	environment.sdfgi_bounce_feedback = 0.3
	environment.ssao_enabled = true
	environment.ssao_radius = 0.7
	environment.ssao_intensity = 0.9
	environment.fog_enabled = true
	environment.fog_light_color = Color("b8cbce")
	environment.fog_density = 0.0025
	var world := WorldEnvironment.new()
	world.environment = environment
	add_child(world)
	actor_meshes.append(_mesh(bridge.actor_geometry(0)))
	actor_meshes.append(_mesh(bridge.actor_geometry(1)))
	item_mesh = _mesh(bridge.item_geometry())

func _mesh(data: Dictionary) -> ArrayMesh:
	var result := ArrayMesh.new()
	if data.is_empty() or data.vertices.is_empty(): return result
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = data.vertices
	arrays[Mesh.ARRAY_NORMAL] = data.normals
	arrays[Mesh.ARRAY_COLOR] = data.colors
	arrays[Mesh.ARRAY_TEX_UV] = data.uv
	arrays[Mesh.ARRAY_TEX_UV2] = data.uv2
	result.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	result.surface_set_material(0, material)
	return result

func _demo() -> void:
	_release_mouse()
	bridge.start_demo()
	hud.inventory_open = false
	yaw = deg_to_rad(float(options.get("yaw",0)))
	pitch = -0.10

func _connect_game(address: String, player_name: String, certificate: String) -> void:
	_release_mouse()
	bridge.connect_game(address,player_name,certificate)
	hud.inventory_open = false

func _disconnect_game() -> void:
	_release_mouse()
	bridge.disconnect_game()
	hud.inventory_open = false

func _release_mouse() -> void:
	captured = false
	attack = false
	pickup = false
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	for action in ["forward","back","left","right","jump"]: Input.action_release(action)
	if is_instance_valid(bridge): bridge.clear_actions()

func _focus_lost() -> void:
	focused = false
	_release_mouse()

func _input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo:
		if event.keycode == KEY_ESCAPE:
			_release_mouse()
			get_viewport().set_input_as_handled()
		elif event.keycode == KEY_TAB:
			hud.inventory_open = not hud.inventory_open
			_release_mouse()
			var focus_owner := get_viewport().gui_get_focus_owner()
			if focus_owner: focus_owner.release_focus()
			get_viewport().set_input_as_handled()

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo:
		if captured and event.physical_keycode == KEY_F: attack = true
		elif captured and event.physical_keycode == KEY_G: pickup = true
	if event is InputEventMouseMotion and captured:
		yaw = fposmod(yaw + event.relative.x * 0.0025,TAU)
		pitch = clampf(pitch - event.relative.y * 0.0025,-1.45,1.45)
	if event is InputEventMouseButton and event.pressed:
		if event.button_index == MOUSE_BUTTON_LEFT and state.get("ready",false) and not hud.inventory_open:
			if captured: attack = true
			else:
				captured = true
				Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
			get_viewport().set_input_as_handled()
		elif captured and event.button_index == MOUSE_BUTTON_WHEEL_UP: hud.fov_slider.value -= 2
		elif captured and event.button_index == MOUSE_BUTTON_WHEEL_DOWN: hud.fov_slider.value += 2

func _process(delta: float) -> void:
	var controls := {"yaw": yaw, "pitch": pitch}
	if captured and focused and not hud.inventory_open:
		controls.forward = Input.get_action_strength("forward") - Input.get_action_strength("back")
		controls.strafe = Input.get_action_strength("right") - Input.get_action_strength("left")
		controls.jump = Input.is_action_pressed("jump")
		controls.attack = attack
		controls.pickup = pickup
	attack = false
	pickup = false
	state = bridge.advance(delta,controls)
	if not state.ready and captured: _release_mouse()
	var next_revision := Vector2i(state.generation,state.revision)
	if revision.x != next_revision.x:
		_clear_world()
	if state.ready and revision != next_revision:
		_rebuild_terrain()
		revision = next_revision
		ready_frames = 0
	if state.ready:
		camera.position = state.position + Vector3(0,1.58,0)
		camera.rotation = Vector3(pitch,-yaw,0)
		_update_entities()
		ready_frames += 1
	hud.update_state(state,captured)
	if not capture_path.is_empty() and not capture_pending:
		if Time.get_ticks_msec() - started > 120000:
			push_error("Capture timed out waiting for world")
			get_tree().quit(1)
		elif state.ready and ready_frames >= capture_frames:
			capture_pending = true
			_capture()

func _clear_world() -> void:
	for child in terrain_root.get_children():
		terrain_root.remove_child(child)
		child.queue_free()
	for node in entities.values(): node.queue_free()
	for node in item_nodes.values(): node.queue_free()
	entities.clear()
	item_nodes.clear()
	revision = Vector2i(state.generation,-1)

func _rebuild_terrain() -> void:
	for child in terrain_root.get_children():
		terrain_root.remove_child(child)
		child.queue_free()
	for y in range(0,32,16):
		for z in range(0,64,16):
			for x in range(0,64,16):
				var mesh := _mesh(bridge.chunk_geometry(Vector3i(x,y,z)))
				if mesh.get_surface_count() == 0: continue
				var node := MeshInstance3D.new()
				node.name = "Chunk_%d_%d_%d" % [x,y,z]
				node.mesh = mesh
				node.gi_mode = GeometryInstance3D.GI_MODE_STATIC
				terrain_root.add_child(node)

func _update_entities() -> void:
	var live := {}
	for actor in state.actors:
		live[actor.id] = true
		if not entities.has(actor.id):
			var node := MeshInstance3D.new()
			node.mesh = actor_meshes[clampi(actor.kind,0,1)]
			node.gi_mode = GeometryInstance3D.GI_MODE_DYNAMIC
			add_child(node)
			entities[actor.id] = node
		entities[actor.id].position = actor.position
		entities[actor.id].rotation.y = -actor.yaw
	for id in entities.keys():
		if not live.has(id):
			entities[id].queue_free()
			entities.erase(id)
	live.clear()
	for item in state.items:
		live[item.id] = true
		if not item_nodes.has(item.id):
			var node := MeshInstance3D.new()
			node.mesh = item_mesh
			add_child(node)
			item_nodes[item.id] = node
		item_nodes[item.id].position = item.position
	for id in item_nodes.keys():
		if not live.has(id):
			item_nodes[id].queue_free()
			item_nodes.erase(id)

func _capture() -> void:
	await RenderingServer.frame_post_draw
	var image := get_viewport().get_texture().get_image()
	var error := image.save_png(capture_path)
	if error != OK: push_error("Capture save failed: %s" % error)
	get_tree().quit(0 if error == OK else 1)

func _exit_tree() -> void:
	_release_mouse()
	if is_instance_valid(bridge): bridge.disconnect_game()
