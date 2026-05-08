extends Node

var client: Node
var tilemap_node: Node2D
var entities_node: Node2D
var camera: Camera2D
var hud: Control

func _ready():
	var cdda_class = ClassDB.class_exists("CddaClient")
	print("CddaClient class exists: ", cdda_class)

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
	hud = get_node_or_null("HUD")

	client.connect_to_game_server("127.0.0.1:9876")

var _tick_count = 0

func _process(delta):
	if not client:
		return

	_tick_count += 1
	client.tick(delta)

	if tilemap_node and tilemap_node.has_method("sync"):
		tilemap_node.call("sync", client)

	if entities_node and entities_node.has_method("sync"):
		entities_node.call("sync", client)

	if camera:
		var pos = client.get_player_pos()
		camera.position = Vector2(pos.x * 16, pos.y * 16)

	if hud and hud.has_method("update"):
		hud.call("update", client)
