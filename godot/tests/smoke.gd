extends SceneTree
var failed := false

func check(condition: bool, message: String) -> void:
	if not condition:
		failed = true
		push_error(message)
	else: print("PASS: ",message)

func _initialize() -> void:
	call_deferred("run")

func run() -> void:
	var scene = load("res://scenes/main.tscn").instantiate()
	root.add_child(scene)
	current_scene = scene
	scene.set_process(false)
	scene._demo()
	var bridge: GameBridge = scene.bridge
	var first := bridge.snapshot()
	check(first.ready and first.demo,"offline world and native extension loaded")
	check(bridge.chunk_geometry(Vector3i(16,0,16)).vertices.size() > 0,"chunk geometry crosses extension boundary")
	check(bridge.chunk_geometry(Vector3i(-16,0,0)).is_empty(),"invalid chunk rejected")
	var start: Vector3 = first.body_position
	var peak := start.y
	for frame in range(600):
		var s := bridge.advance(1.0/120.0,{"forward":1.0,"jump":frame < 4})
		peak = maxf(peak,s.body_position.y)
	var last := bridge.snapshot()
	check(peak > start.y + 1.0 and last.grounded,"jump rises and lands through Rust simulation")
	check(last.body_position.z > 19.0 and last.body_position.z < 19.4,"stone wall stops native simulated movement")
	var before: Vector3 = last.body_position
	bridge.advance(0.1,{"forward":99.0})
	check(bridge.snapshot().body_position == before,"invalid bridge input cannot move player")
	scene._demo()
	scene._process(0.0)
	check(scene.terrain_root.get_child_count() > 0,"Godot terrain scene built")
	check(scene.entities.size() == 1 and scene.item_nodes.size() == 1,"actors and ground items displayed")
	scene.captured = true
	scene.focused = true
	Input.action_press("forward")
	scene._process(0.05)
	check(scene.state.body_position.z < start.z,"Godot input reaches Rust prediction")
	scene._focus_lost()
	check(not scene.captured and not Input.is_action_pressed("forward"),"focus loss releases mouse and movement")
	var key := InputEventKey.new()
	key.pressed = true
	key.keycode = KEY_TAB
	scene._input(key)
	scene._process(0.05)
	check(scene.hud.inventory_open and scene.hud.panel.visible,"inventory UI opens and releases movement")
	scene.hud.chat.grab_focus()
	scene._input(key)
	check(not scene.hud.inventory_open,"Tab closes inventory even with a focused text field")
	scene.hud.fov_slider.value = 85
	check(scene.camera.fov == 85,"FOV control updates camera")
	scene._disconnect_game()
	scene._process(0.0)
	check(not scene.state.ready and scene.entities.is_empty(),"disconnect clears scene and session")
	scene._demo()
	scene._process(0.0)
	check(scene.camera.position.distance_to(Vector3(28.5,8.58,32.5)) < 0.001,"new session resets camera interpolation")
	scene.queue_free()
	await process_frame
	print("GODOT_SMOKE_RESULT=", "FAIL" if failed else "PASS")
	quit(1 if failed else 0)
