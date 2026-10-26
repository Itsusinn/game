extends Control
signal singleplayer_requested
signal connect_requested(address: String, player_name: String, certificate: String)
signal resume_requested
signal home_requested
signal quit_requested
signal fov_changed(value: float)
signal lighting_changed(enabled: bool)

const ACCENT := Color("d7ec9b")
var page := "home"
var return_page := "home"
var in_game := false
var address: LineEdit
var player_name: LineEdit
var certificate: LineEdit
var fov_slider: HSlider
var gi_toggle: CheckButton
var sensitivity: HSlider
var fullscreen: CheckButton
var pages: Dictionary = {}
var heading: Label
var subtitle: Label
var feedback: Label
var hero: VBoxContainer
var card: PanelContainer
var settings := ConfigFile.new()
var loading_settings := true
var transition: Tween

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_STOP
	var ui_theme := Theme.new()
	var font := SystemFont.new()
	font.font_names = PackedStringArray(["Microsoft YaHei UI", "Noto Sans CJK SC"])
	ui_theme.default_font = font
	ui_theme.default_font_size = 18
	ui_theme.set_color("font_color", "Label", Color("e5ece9"))
	for kind in ["normal", "hover", "pressed", "focus", "disabled"]:
		var style := StyleBoxFlat.new()
		style.bg_color = Color("1b2a30") if kind == "normal" else Color("31464b")
		if kind == "disabled": style.bg_color = Color("182127")
		style.border_color = ACCENT if kind in ["hover", "focus"] else Color("4a6065")
		style.set_border_width_all(1)
		style.content_margin_left = 22
		style.content_margin_right = 22
		ui_theme.set_stylebox(kind, "Button", style)
	ui_theme.set_color("font_hover_color", "Button", ACCENT)
	ui_theme.set_color("font_focus_color", "Button", ACCENT)
	theme = ui_theme
	var header := Label.new()
	header.text = "D / 3D     ·     VOXEL FRONTIER"
	header.position = Vector2(56, 30)
	header.add_theme_font_size_override("font_size", 18)
	add_child(header)
	hero = VBoxContainer.new()
	hero.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	hero.anchor_left = 0.075
	hero.anchor_right = 0.52
	hero.anchor_top = 0.29
	hero.anchor_bottom = 0.78
	hero.add_theme_constant_override("separation", 20)
	hero.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(hero)
	_text(hero, "EXPEDITION  /  001", 16).modulate = ACCENT
	_text(hero, "方块边境", 66)
	_text(hero, "D U N G E O N   /   3 D", 21)
	_text(hero, "越过石墙，探索未知。\n你的下一段旅程，从这里开始。", 19)
	_text(hero, "探索  /  生存  /  并肩作战", 15).modulate = Color("94aaa9")
	card = PanelContainer.new()
	card.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	card.anchor_left = 0.58
	card.anchor_right = 0.94
	card.anchor_top = 0.16
	card.anchor_bottom = 0.87
	var panel_style := StyleBoxFlat.new()
	panel_style.bg_color = Color(0.055, 0.085, 0.10, 0.94)
	panel_style.border_color = Color("4b6064")
	panel_style.border_width_top = 3
	panel_style.content_margin_left = 28
	panel_style.content_margin_right = 28
	panel_style.content_margin_top = 24
	panel_style.content_margin_bottom = 22
	card.add_theme_stylebox_override("panel", panel_style)
	add_child(card)
	var scroll := ScrollContainer.new()
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	card.add_child(scroll)
	var content := VBoxContainer.new()
	content.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	content.add_theme_constant_override("separation", 12)
	scroll.add_child(content)
	heading = _text(content, "开始旅程", 28)
	subtitle = _text(content, "选择你的探索方式", 15)
	subtitle.modulate = Color("9fb1b4")
	content.add_child(HSeparator.new())
	for key in ["home", "multiplayer", "settings", "pause", "busy"]:
		var column := VBoxContainer.new()
		column.add_theme_constant_override("separation", 12)
		content.add_child(column)
		pages[key] = column
	var home: VBoxContainer = pages.home
	var single := _button(home, "01   单人游戏     →", func(): singleplayer_requested.emit())
	single.add_theme_color_override("font_color", ACCENT)
	single.custom_minimum_size.y = 76
	_button(home, "02   多人游戏", func(): show_page("multiplayer"))
	_button(home, "03   设置", open_settings)
	_button(home, "04   退出游戏", func(): quit_requested.emit())
	var multi: VBoxContainer = pages.multiplayer
	address = _field(multi, "服务器地址", "127.0.0.1:9876")
	player_name = _field(multi, "玩家名称", "Explorer")
	certificate = _field(multi, "服务器证书 (.der)", ProjectSettings.globalize_path("res://../save/server.cert.der"))
	_button(multi, "连接服务器  →", func(): connect_requested.emit(address.text.strip_edges(), player_name.text.strip_edges(), certificate.text.strip_edges()))
	_button(multi, "返回", func(): show_page("home"))
	var config: VBoxContainer = pages.settings
	_text(config, "视野范围", 16)
	fov_slider = HSlider.new()
	fov_slider.min_value = 55
	fov_slider.max_value = 100
	fov_slider.value = 72
	fov_slider.custom_minimum_size.y = 30
	config.add_child(fov_slider)
	var fov_value := _text(config, "72°", 14)
	fov_slider.value_changed.connect(func(value: float):
		fov_value.text = "%d°" % value
		fov_changed.emit(value)
		_save_settings())
	_text(config, "鼠标灵敏度", 16)
	sensitivity = HSlider.new()
	sensitivity.min_value = 0.5
	sensitivity.max_value = 2.0
	sensitivity.step = 0.05
	sensitivity.value = 1.0
	sensitivity.custom_minimum_size.y = 30
	config.add_child(sensitivity)
	sensitivity.value_changed.connect(func(_value: float): _save_settings())
	gi_toggle = CheckButton.new()
	gi_toggle.text = "全局光照"
	gi_toggle.button_pressed = true
	config.add_child(gi_toggle)
	gi_toggle.toggled.connect(func(enabled: bool):
		lighting_changed.emit(enabled)
		_save_settings())
	fullscreen = CheckButton.new()
	fullscreen.text = "全屏显示"
	config.add_child(fullscreen)
	fullscreen.toggled.connect(func(enabled: bool):
		DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_FULLSCREEN if enabled else DisplayServer.WINDOW_MODE_WINDOWED)
		_save_settings())
	_button(config, "返回", func(): show_page(return_page))
	var pause: VBoxContainer = pages.pause
	_button(pause, "继续游戏  →", func(): resume_requested.emit())
	_button(pause, "设置", open_settings)
	_button(pause, "返回主界面", func(): home_requested.emit())
	_button(pause, "退出游戏", func(): quit_requested.emit())
	_text(pages.busy, "请稍候…", 20)
	_button(pages.busy, "返回主界面", func(): home_requested.emit())
	feedback = _text(content, "", 14)
	feedback.modulate = ACCENT
	var footer := Label.new()
	footer.text = "WASD  移动     /     SPACE  跳跃     /     ESC  游戏菜单"
	footer.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_WIDE)
	footer.offset_left = 56
	footer.offset_top = -48
	footer.add_theme_font_size_override("font_size", 14)
	footer.modulate = Color("a8b8b6")
	add_child(footer)
	settings.load("user://settings.cfg")
	fov_slider.value = clampf(float(settings.get_value("video", "fov", 72)), 55, 100)
	sensitivity.value = clampf(float(settings.get_value("input", "sensitivity", 1.0)), 0.5, 2.0)
	gi_toggle.button_pressed = bool(settings.get_value("video", "gi", true))
	fullscreen.button_pressed = bool(settings.get_value("video", "fullscreen", false))
	loading_settings = false
	show_page("home")
	resized.connect(queue_redraw)

func _draw() -> void:
	draw_rect(Rect2(Vector2.ZERO, size), Color(0.025, 0.045, 0.06, 0.91 if not in_game else 0.78))
	if in_game: return
	# Restrained topographic lines keep the title legible and echo the voxel world.
	for i in range(13):
		var y := size.y * 0.49 + i * 24.0
		var points := PackedVector2Array([Vector2(0,y+80), Vector2(size.x*0.17,y+80), Vector2(size.x*0.29,y-24), Vector2(size.x*0.43,y-24), Vector2(size.x*0.54,y-95)])
		draw_polyline(points, Color(0.30,0.48,0.46,0.16), 1.0, true)
	draw_line(Vector2(56,80), Vector2(size.x-56,80), Color("405357"), 1)
	draw_line(Vector2(56,size.y-70), Vector2(size.x-56,size.y-70), Color("405357"), 1)

func show_page(next: String) -> void:
	page = next
	visible = true
	for key in pages: pages[key].visible = key == page
	heading.text = {"home":"开始旅程", "multiplayer":"多人游戏", "settings":"设置", "pause":"游戏菜单", "busy":"正在处理"}[page]
	subtitle.text = {"home":"选择你的探索方式", "multiplayer":"与伙伴一起探索边境", "settings":"调整你的视野与操作", "pause":"世界仍在运行，请注意周围", "busy":"正在准备或关闭世界"}[page]
	feedback.text = ""
	hero.visible = not in_game
	if transition: transition.kill()
	card.modulate.a = 0.4
	transition = create_tween()
	transition.tween_property(card, "modulate:a", 1.0, 0.16)
	var first: Array[Node] = pages[page].find_children("*", "Button", true, false)
	if not first.is_empty(): first[0].grab_focus()
	queue_redraw()

func open_settings() -> void:
	return_page = "pause" if in_game else "home"
	show_page("settings")

func _save_settings() -> void:
	if loading_settings: return
	settings.set_value("video", "fov", fov_slider.value)
	settings.set_value("video", "gi", gi_toggle.button_pressed)
	settings.set_value("video", "fullscreen", fullscreen.button_pressed)
	settings.set_value("input", "sensitivity", sensitivity.value)
	if settings.save("user://settings.cfg") != OK: feedback.text = "设置保存失败，请检查用户目录权限。"

func _text(parent: Node, text: String, font_size: int) -> Label:
	var result := Label.new()
	result.text = text
	result.add_theme_font_size_override("font_size", font_size)
	result.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	parent.add_child(result)
	return result

func _button(parent: Node, text: String, callback: Callable) -> Button:
	var result := Button.new()
	result.text = text
	result.alignment = HORIZONTAL_ALIGNMENT_LEFT
	result.custom_minimum_size.y = 54
	parent.add_child(result)
	result.pressed.connect(callback)
	return result

func _field(parent: Node, caption: String, text: String) -> LineEdit:
	_text(parent, caption, 14)
	var result := LineEdit.new()
	result.text = text
	result.custom_minimum_size.y = 38
	parent.add_child(result)
	return result
