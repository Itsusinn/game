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
const Palette = preload("res://src/interface_theme.gd")
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
var health_bar: ProgressBar

func _ready() -> void:
	var theme := Palette.create()
	var root := Control.new()
	root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	root.theme = theme
	add_child(root)
	top = PanelContainer.new()
	top.set_anchors_and_offsets_preset(Control.PRESET_TOP_WIDE)
	top.offset_left = 18
	top.offset_right = -18
	top.offset_top = 16
	top.offset_bottom = 80
	var bar_style := Palette.box(Color(0.055,0.065,0.08,0.92), 14)
	bar_style.border_width_bottom = 1
	bar_style.border_color = Color(1,1,1,0.2)
	top.add_theme_stylebox_override("panel",bar_style)
	root.add_child(top)
	var title := HBoxContainer.new()
	title.add_theme_constant_override("separation", 30)
	top.add_child(title)
	label(title, "方块边境", 25).add_theme_font_override("font",Palette.display_font())
	label(title, "/  FIELD OPERATIONS", 14).modulate = Palette.MUTED
	health = label(title, "", 17)
	health.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	health.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
	health.add_theme_font_override("font",Palette.technical_font())
	health_bar = ProgressBar.new()
	health_bar.custom_minimum_size = Vector2(120,6)
	health_bar.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	health_bar.show_percentage = false
	health_bar.add_theme_stylebox_override("background",Palette.box(Color("515b64"),0))
	health_bar.add_theme_stylebox_override("fill",Palette.box(Palette.ACCENT,0))
	title.add_child(health_bar)
	bottom = PanelContainer.new()
	bottom.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_WIDE)
	bottom.offset_left = 18
	bottom.offset_right = -18
	bottom.offset_top = -90
	bottom.offset_bottom = -16
	bottom.add_theme_stylebox_override("panel",Palette.box(Color(0.055,0.065,0.08,0.92),12))
	root.add_child(bottom)
	var footer := VBoxContainer.new()
	bottom.add_child(footer)
	status = label(footer, "准备就绪", 14)
	status.modulate = Palette.ACCENT
	label(footer, "WASD  移动    /    SPACE  跳跃    /    F  近战    /    G  拾取    /    TAB  行囊    /    ESC  菜单", 14)
	panel = PanelContainer.new()
	panel.set_anchors_and_offsets_preset(Control.PRESET_RIGHT_WIDE)
	panel.offset_left = -390
	panel.offset_right = -18
	panel.offset_top = 100
	panel.offset_bottom = -106
	root.add_child(panel)
	var scroll := ScrollContainer.new()
	panel.add_child(scroll)
	var column := VBoxContainer.new()
	column.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	column.add_theme_constant_override("separation", 10)
	scroll.add_child(column)
	label(column, "行囊  /  INVENTORY", 23).add_theme_font_override("font",Palette.display_font())
	inventory = label(column, "行囊中暂无物品。", 15)
	inventory.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	column.add_child(HSeparator.new())
	label(column, "行动记录  /  LOG", 21).add_theme_font_override("font",Palette.display_font())
	journal = label(column, "", 14)
	journal.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	chat = field(column, "通讯  /  CHAT", "")
	chat.placeholder_text = "输入消息，按 Enter 发送"
	chat.text_submitted.connect(func(text: String):
		if not text.strip_edges().is_empty():
			chat_requested.emit(text)
			chat.clear())
	coordinates = label(column, "", 14)
	reticle = label(root, "+", 25)
	reticle.add_theme_color_override("font_color",Color.WHITE)
	reticle.set_anchors_and_offsets_preset(Control.PRESET_CENTER)
	reticle.mouse_filter = Control.MOUSE_FILTER_IGNORE
	hint = label(root, "点击画面，开始行动", 16)
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
	health.text = "HP   %03d / 100" % state.hp if state.ready else ""
	health_bar.value = state.hp
	status.text = "离线漫游  /  EXPLORATION" if state.demo else ("单人行动  /  SOLO" if state.get("local_running",false) else "多人行动  /  MULTIPLAYER")
	if not state.ready: status.text = state.status
	var p: Vector3 = state.body_position
	coordinates.text = "X %.1f   Y %.1f   Z %.1f" % [p.x, p.y, p.z]
	var rows := PackedStringArray()
	for item in state.inventory:
		rows.append("%s  x%d" % [item.name, item.quantity])
	inventory.text = "行囊中暂无物品。" if rows.is_empty() else "\n".join(rows)
	journal.text = "\n".join(PackedStringArray(state.journal))
	chat.editable = state.ready and not state.demo
	reticle.visible = state.ready and not panel.visible and not menu.visible
	hint.visible = state.ready and not captured and not panel.visible and not menu.visible
