extends RefCounted

const INK := Color("171b20")
const PAPER := Color("eef0ed")
const ACCENT := Color("ff782b")
const BLUE := Color("079fce")
const MUTED := Color("afb8c0")

static func body_font() -> SystemFont:
	var font := SystemFont.new()
	font.font_names = PackedStringArray(["Segoe UI", "Microsoft YaHei UI", "Noto Sans CJK SC"])
	return font

static func display_font() -> SystemFont:
	var font := body_font()
	font.font_weight = 600
	return font

static func technical_font() -> SystemFont:
	return body_font()

static func box(color: Color, margin: int = 16) -> StyleBoxFlat:
	var style := StyleBoxFlat.new()
	style.bg_color = color
	style.set_corner_radius_all(12)
	style.content_margin_left = margin
	style.content_margin_right = margin
	style.content_margin_top = margin
	style.content_margin_bottom = margin
	return style

static func create() -> Theme:
	var result := Theme.new()
	result.default_font = body_font()
	result.default_font_size = 17
	result.set_color("font_color", "Label", PAPER)
	result.set_color("font_color", "CheckButton", PAPER)
	result.set_color("font_hover_color", "CheckButton", ACCENT)
	var panel := box(Color(0.065,0.075,0.09,0.95), 28)
	panel.set_corner_radius_all(22)
	panel.set_border_width_all(1)
	panel.border_color = Color(1,1,1,0.25)
	panel.shadow_color = Color(0,0,0,0.3)
	panel.shadow_size = 24
	panel.shadow_offset = Vector2(0,12)
	result.set_stylebox("panel", "PanelContainer", panel)
	for state in ["normal", "hover", "pressed", "disabled", "focus"]:
		var style := box(PAPER, 16)
		style.set_border_width_all(1)
		style.border_color = Color(1,1,1,0.85)
		style.shadow_size = 5
		style.shadow_offset = Vector2(0,3)
		style.shadow_color = Color(0,0,0,0.2)
		if state == "hover": style.bg_color = Color(1,1,1,0.95)
		if state == "pressed": style.bg_color = Color("c7d0d2")
		if state == "disabled": style.bg_color = Color("7b858b")
		if state == "focus":
			style.bg_color = Color.TRANSPARENT
			style.border_color = ACCENT
			style.set_border_width_all(2)
			style.shadow_size = 0
		result.set_stylebox(state, "Button", style)
		var toggle_style := style.duplicate() as StyleBoxFlat
		if state != "focus":
			toggle_style.bg_color = Color(0.16,0.19,0.22,0.9)
			toggle_style.border_color = Color(1,1,1,0.15)
		result.set_stylebox(state, "CheckButton", toggle_style)
	for state in ["font_color", "font_hover_color", "font_pressed_color", "font_focus_color"]:
		result.set_color(state, "Button", INK)
		result.set_color(state, "CheckButton", PAPER)
	for state in ["normal", "focus", "read_only"]:
		var field := box(Color("252c33"), 10)
		field.border_width_bottom = 2
		field.border_color = BLUE if state == "focus" else Color("7c878e")
		result.set_stylebox(state, "LineEdit", field)
	result.set_color("font_color", "LineEdit", PAPER)
	result.set_color("caret_color", "LineEdit", ACCENT)
	result.set_stylebox("slider", "HSlider", box(Color("626c73"), 3))
	result.set_stylebox("grabber_area", "HSlider", box(BLUE, 3))
	result.set_stylebox("grabber_area_highlight", "HSlider", box(ACCENT, 3))
	return result
