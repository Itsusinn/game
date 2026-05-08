extends SceneTree

var client: Node

func _initialize():
	print("CddaClient class exists: ", ClassDB.class_exists("CddaClient"))

	if not ClassDB.class_exists("CddaClient"):
		print("ERROR: CddaClient not found - GDExtension not loaded!")
		quit(1)
		return

	client = ClassDB.instantiate("CddaClient")
	if not client:
		print("ERROR: Failed to instantiate CddaClient")
		quit(1)
		return

	print("CddaClient instantiated successfully!")

	root.add_child(client)
	client.connect_to_game_server("127.0.0.1:9876")

	# Wait a bit for connection
	await create_timer(2.0).timeout

	# Check state
	var pos = client.get_player_pos()
	var hp = client.get_player_hp()
	var tiles = client.get_visible_tile_count()
	var entities = client.get_entity_count()
	print("Player pos: (%d, %d)" % [pos.x, pos.y])
	print("HP: %d" % hp)
	print("Visible tiles: %d" % tiles)
	print("Entities: %d" % entities)
	print("Game connected: %s" % client.is_game_connected())

	# Test tick
	for i in range(5):
		client.tick(0.016)
		await create_timer(0.3).timeout

	# Send a move
	client.send_move(3)  # MoveRight
	print("Sent MoveRight")

	await create_timer(0.5).timeout
	client.tick(0.016)

	pos = client.get_player_pos()
	print("Player pos after move: (%d, %d)" % [pos.x, pos.y])

	# Disconnect
	client.disconnect_from_server()
	print("Test complete!")

	await create_timer(0.5).timeout
	quit(0)
