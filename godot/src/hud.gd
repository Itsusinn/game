extends CanvasLayer
signal connect_requested(address: String, player_name: String, certificate: String)
signal demo_requested
signal disconnect_requested
signal chat_requested(text: String)
signal fov_changed(value: float)
signal lighting_changed(enabled: bool)
signal singleplayer_requested
signal resume_requested
signal home_requested
signal quit_requested

const MenuScript = preload("res://src/menu.gd")
var menu: Control
var top: PanelContainer
var bottom: PanelContainer

var panel: PanelContainer
var address: LineEdit
var player_name: LineEdit
var certificate: LineEdit
var chat: LineEdit
var status: Label
var health: Label
var coordinates: Label
var inventory: Label
var journal: Label
var reticle: Label
var hint: Label
var fov_slider: HSlider
var gi_toggle: CheckButton
var inventory_open := false

func _ready() -> void:
	var theme := Theme.new()
	theme.default_font_size = 16
	var style := StyleBoxFlat.new()
	style.bg_color = Color("18251f", 0.96)
	style.border_color = Color("53654e")
	style.set_border_width_all(1)
	style.set_corner_radius_all(6)
	style.content_margin_left = 18
	style.content_margin_right = 18
	style.content_margin_top = 12
	style.content_margin_bottom = 12
	theme.set_stylebox("panel", "PanelContainer", style)
	var root := Control.new()
	root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	root.theme = theme
	add_child(root)
	top = PanelContainer.new()
	top.set_anchors_and_offsets_preset(Control.PRESET_TOP_WIDE)
	top.offset_bottom = 58
	root.add_child(top)
	var title := HBoxContainer.new()
	title.add_theme_constant_override("separation", 30)
	top.add_child(title)
	label(title, "D U N G E O N   /   3 D", 23).modulate = Color("cee4b2")
	label(title, "VOXEL FRONTIER", 14)
	health = label(title, "", 17)
	health.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	health.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
	bottom = PanelContainer.new()
	bottom.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_WIDE)
	bottom.offset_top = -74
	root.add_child(bottom)
	var footer := VBoxContainer.new()
	bottom.add_child(footer)
	status = label(footer, "Ready", 14)
	label(footer, "WASD / Arrows  Move     Space  Jump     Click / F  Attack     G  Pick up     Tab  Pack     Esc  Menu", 14)
	panel = PanelContainer.new()
	panel.set_anchors_and_offsets_preset(Control.PRESET_RIGHT_WIDE)
	panel.offset_left = -340
	panel.offset_right = -18
	panel.offset_top = 78
	panel.offset_bottom = -92
	root.add_child(panel)
	var scroll := ScrollContainer.new()
	panel.add_child(scroll)
	var column := VBoxContainer.new()
	column.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	column.add_theme_constant_override("separation", 10)
	scroll.add_child(column)
	label(column, "Inventory", 19)
	inventory = label(column, "Your pack is empty.", 15)
	inventory.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	label(column, "Journal", 19)
	journal = label(column, "", 14)
	journal.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	chat = field(column, "Chat", "")
	chat.placeholder_text = "Enter to send"
	chat.text_submitted.connect(func(text: String):
		if not text.strip_edges().is_empty():
			chat_requested.emit(text)
			chat.clear())
	coordinates = label(column, "", 14)
	reticle = label(root, "+", 25)
	reticle.set_anchors_and_offsets_preset(Control.PRESET_CENTER)
	reticle.mouse_filter = Control.MOUSE_FILTER_IGNORE
	hint = label(root, "Click the world to explore", 16)
	hint.set_anchors_and_offsets_preset(Control.PRESET_CENTER)
	hint.position += Vector2(-90, 38)
	hint.mouse_filter = Control.MOUSE_FILTER_IGNORE
	menu = MenuScript.new()
	root.add_child(menu)
	address = menu.address
	player_name = menu.player_name
	certificate = menu.certificate
	fov_slider = menu.fov_slider
	gi_toggle = menu.gi_toggle
	menu.connect_requested.connect(func(a: String, n: String, c: String): connect_requested.emit(a,n,c))
	menu.singleplayer_requested.connect(func(): singleplayer_requested.emit())
	menu.resume_requested.connect(func(): resume_requested.emit())
	menu.home_requested.connect(func(): home_requested.emit())
	menu.quit_requested.connect(func(): quit_requested.emit())
	menu.fov_changed.connect(func(value: float): fov_changed.emit(value))
	menu.lighting_changed.connect(func(enabled: bool): lighting_changed.emit(enabled))

func label(parent: Node, text: String, size: int) -> Label:
	var result := Label.new()
	result.text = text
	result.add_theme_font_size_override("font_size", size)
	parent.add_child(result)
	return result

func field(parent: Node, caption: String, text: String) -> LineEdit:
	label(parent, caption, 14)
	var result := LineEdit.new()
	result.text = text
	parent.add_child(result)
	return result

func button(parent: Node, text: String, callback: Callable) -> Button:
	var result := Button.new()
	result.text = text
	result.custom_minimum_size.y = 34
	parent.add_child(result)
	result.pressed.connect(callback)
	return result

func update_state(state: Dictionary, captured: bool) -> void:
	panel.visible = inventory_open and state.ready and not menu.visible
	top.visible = state.ready and not menu.visible
	bottom.visible = top.visible
	health.text = "HEALTH  %d / 100" % state.hp if state.ready else ""
	status.text = "%s   /   %d pending inputs" % [state.status, state.pending]
	var p: Vector3 = state.body_position
	coordinates.text = "X %.1f   Y %.1f   Z %.1f" % [p.x, p.y, p.z]
	var rows := PackedStringArray()
	for item in state.inventory:
		rows.append("%s  x%d" % [item.name, item.quantity])
	inventory.text = "Your pack is empty." if rows.is_empty() else "\n".join(rows)
	journal.text = "\n".join(PackedStringArray(state.journal))
	chat.editable = state.ready and not state.demo
	reticle.visible = state.ready and not panel.visible and not menu.visible
	hint.visible = state.ready and not captured and not panel.visible and not menu.visible
