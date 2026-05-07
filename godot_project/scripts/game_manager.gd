extends Node

var client: RefCounted
var tilemap_node: Node2D
var entities_node: Node2D
var camera: Camera2D
var hud: Control

func _ready():
	if not ClassDB.class_exists("CddaClient"):
		push_error("GDExtension not loaded: CddaClient class not found")
		return

	client = CddaClient.new()
	add_child(client)

	tilemap_node = get_node_or_null("Map/TileMap")
	entities_node = get_node_or_null("Map/Entities")
	camera = get_node_or_null("Camera")
	hud = get_node_or_null("HUD")

	client.connect_to_server("127.0.0.1:9876")

func _process(delta):
	if not client or not client.is_connected():
		return

	client.tick(delta)

	# Sync tilemap
	if tilemap_node and tilemap_node.has_method("sync"):
		tilemap_node.call("sync", client)

	# Sync entities
	if entities_node and entities_node.has_method("sync"):
		entities_node.call("sync", client)

	# Update camera
	if camera:
		var pos = client.get_player_pos()
		camera.position = Vector2(pos.x * 16, pos.y * 16)

	# Update HUD
	if hud and hud.has_method("update"):
		hud.call("update", client)
