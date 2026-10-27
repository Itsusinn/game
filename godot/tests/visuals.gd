extends SceneTree
# Run with the graphical renderer. Captures use the offline demo, never real saves.
var scene: Node3D
var output: String

func _initialize() -> void: call_deferred("run")

func snapshot(file: String) -> void:
	for i in range(16): await process_frame
	await RenderingServer.frame_post_draw
	var result := root.get_texture().get_image().save_png(output.path_join(file))
	if result != OK:
		push_error("Cannot write visual evidence: " + file)
		quit(1)

func run() -> void:
	output = OS.get_environment("CDDA_VISUAL_PATH")
	if output.is_empty(): output = ProjectSettings.globalize_path("res://../target/visual-review")
	DirAccess.make_dir_recursive_absolute(output)
	scene = load("res://scenes/main.tscn").instantiate()
	root.add_child(scene)
	current_scene = scene
	scene.hud.menu.loading_settings = true
	scene.hud.gi_toggle.button_pressed = true
	scene.hud.fov_slider.value = 72
	scene._demo()
	scene.yaw = -0.28
	scene.pitch = -0.06
	for i in range(90): await process_frame
	scene.hud.visible = false
	await snapshot("background.png")
	scene.hud.visible = true
	await snapshot("world.png")
	scene.hud.inventory_open = true
	await snapshot("inventory.png")
	scene._close_inventory()
	scene.hud.menu.in_game = true
	scene.hud.menu.show_page("pause")
	await snapshot("pause.png")
	scene._disconnect_game()
	for i in range(2): await process_frame
	scene.hud.menu.in_game = false
	scene.hud.menu.show_page("home")
	await snapshot("menu.png")
	scene.hud.menu.open_settings()
	await snapshot("settings.png")
	print("VISUAL_REVIEW_PATH=",output)
	scene.queue_free()
	await process_frame
	quit()
