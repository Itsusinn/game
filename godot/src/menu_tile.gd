extends Button

const Palette = preload("res://src/interface_theme.gd")
const Surface = preload("res://src/tile_surface.gdshader")
var caption := ""
var description := ""
var large := false
var tint := Palette.PAPER
var hover_amount := 0.0
var surface: ColorRect
var ink_layer: Control
var surface_material: ShaderMaterial
var display := Palette.display_font()
var body := Palette.body_font()

func _ready() -> void:
	mouse_default_cursor_shape = Control.CURSOR_POINTING_HAND
	# Preserve native hit testing, keyboard activation and accessibility.
	for state in ["normal", "hover", "pressed", "disabled", "focus"]:
		add_theme_stylebox_override(state, StyleBoxEmpty.new())
	for state in ["font_color", "font_hover_color", "font_pressed_color", "font_focus_color"]:
		add_theme_color_override(state, Color.TRANSPARENT)
	text = caption
	surface = ColorRect.new()
	surface.mouse_filter = Control.MOUSE_FILTER_IGNORE
	surface.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	surface.offset_left = -32
	surface.offset_top = -32
	surface.offset_right = 32
	surface.offset_bottom = 32
	surface_material = ShaderMaterial.new()
	surface_material.shader = Surface
	surface_material.set_shader_parameter("tint", tint)
	surface.material = surface_material
	add_child(surface)
	ink_layer = Control.new()
	ink_layer.mouse_filter = Control.MOUSE_FILTER_IGNORE
	ink_layer.draw.connect(_draw_content)
	add_child(ink_layer)
	resized.connect(_resize)
	_resize()

func _resize() -> void:
	surface_material.set_shader_parameter("panel_size", size)
	ink_layer.queue_redraw()

func _process(delta: float) -> void:
	if not is_visible_in_tree(): return
	var target := 1.0 if is_hovered() or has_focus() else 0.0
	if is_pressed(): target = -0.3
	hover_amount = lerpf(hover_amount,target,1.0-exp(-delta*14.0))
	surface_material.set_shader_parameter("elevation", hover_amount)
	surface_material.set_shader_parameter("pointer", get_local_mouse_position())
	ink_layer.position.y = -hover_amount
	ink_layer.queue_redraw()

func _draw_content() -> void:
	var ink := Palette.PAPER
	var muted := Color(ink,0.72)
	var left := 30.0
	var title_size := 28 if large else 22
	var baseline := 67.0 if large else 48.0
	ink_layer.draw_string(display,Vector2(left,baseline),caption,HORIZONTAL_ALIGNMENT_LEFT,-1,title_size,ink)
	ink_layer.draw_string(body,Vector2(left,baseline+27),description,HORIZONTAL_ALIGNMENT_LEFT,-1,14,muted)
	if large:
		ink_layer.draw_string(body,Vector2(left,size.y-23),"独自探索，自由启程",HORIZONTAL_ALIGNMENT_LEFT,-1,13,Palette.ACCENT)
	if large or size.x > 350:
		var c := Vector2(size.x-54,size.y*0.5)
		ink_layer.draw_line(c-Vector2(6,0),c+Vector2(6,0),Palette.ACCENT,2,true)
		ink_layer.draw_polyline(PackedVector2Array([c+Vector2(1,-5),c+Vector2(6,0),c+Vector2(1,5)]),Palette.ACCENT,2,true)
	if has_focus():
		var focus := Palette.box(Color.TRANSPARENT,0)
		focus.set_corner_radius_all(3)
		focus.set_border_width_all(1)
		focus.border_color = Color(Palette.ACCENT,0.65)
		ink_layer.draw_style_box(focus,Rect2(Vector2.ZERO,size))
