extends Node

var client: Node
var tilemap_node: Node2D
var entities_node: Node2D
var camera: Camera2D
var hud: Control
var input_handler: Node
var connected := false

func _ready():
	var cdda_class = ClassDB.class_exists("CddaClient")
	if not cdda_class:
		push_error("GDExtension not loaded: CddaClient class not found")
		return

	client = ClassDB.instantiate("CddaClient")
	if not client:
		push_error("Failed to instantiate CddaClient")
		return
	add_child(client)

	tilemap_node = get_node_or_null("Map/TileMap")
	entities_node = get_node_or_null("Map/Entities")
	camera = get_node_or_null("Camera")
	_reparent_node("InputHandler", self)
	input_handler = get_node_or_null("InputHandler")
	hud = get_node_or_null("HUD")

	client.connect_to_game_server(Global.get_addr())
	connected = true

func _process(delta):
	if not connected or not client:
		return

	client.tick(delta)

	if tilemap_node and tilemap_node.has_method("sync"):
		tilemap_node.call("sync", client)
	if entities_node and entities_node.has_method("sync"):
		entities_node.call("sync", client)

	if camera:
		var pos = client.get_player_pos()
		const TILE_SIZE := 16
		camera.position = Vector2(
			pos.x * TILE_SIZE + TILE_SIZE * 0.5,
			pos.y * TILE_SIZE + TILE_SIZE * 0.5,
		)

	if hud and hud.has_method("update"):
		hud.call("update", client)

func _reparent_node(path: String, new_parent: Node):
	var node = get_node_or_null(path)
	if node and node.get_parent() != new_parent:
		remove_child(node)
		new_parent.add_child(node)
		node.owner = new_parent

func _notification(what: int):
	if what == NOTIFICATION_WM_CLOSE_REQUEST and connected and client:
		client.disconnect_from_server()
