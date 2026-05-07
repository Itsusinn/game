extends Node

func _input(event):
	var c = get_parent().client
	if not c or not c.is_connected():
		return

	if event is InputEventKey and event.pressed:
		match event.keycode:
			KEY_W, KEY_UP:    c.send_move(0)
			KEY_S, KEY_DOWN:  c.send_move(1)
			KEY_A, KEY_LEFT:  c.send_move(2)
			KEY_D, KEY_RIGHT: c.send_move(3)
			KEY_PERIOD:       c.send_wait()
			KEY_ESCAPE:       c.disconnect()
