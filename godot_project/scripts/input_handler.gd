extends Node

var client_cache: Node

func _ready():
	client_cache = get_parent().get_node_or_null("CddaClient")
	if not client_cache:
		client_cache = get_parent().get_node_or_null("GameClient")

func _input(event):
	if not event is InputEventKey or not event.pressed:
		return

	var c = _find_client()
	if not c or not c.is_game_connected():
		return

	match event.keycode:
		KEY_W, KEY_UP:    c.send_move(0)
		KEY_S, KEY_DOWN:  c.send_move(1)
		KEY_A, KEY_LEFT:  c.send_move(2)
		KEY_D, KEY_RIGHT: c.send_move(3)
		KEY_PERIOD:       c.send_wait()
		KEY_ESCAPE:
			c.disconnect_from_server()
			get_tree().change_scene_to_file("res://scenes/main_menu.tscn")

func _find_client() -> Node:
	return client_cache
