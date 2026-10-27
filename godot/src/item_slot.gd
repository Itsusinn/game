extends Control
signal activated(area: String, index: int, mouse_button: int, quick: bool)
signal dropped(area: String, index: int, payload: Dictionary)
var area := "player"
var index := 0
var item: Dictionary = {}
var payload: Dictionary = {}
var selected := false
var picked := false
var number := ""

func _ready() -> void:
	custom_minimum_size = Vector2(52,52)
	mouse_default_cursor_shape = Control.CURSOR_POINTING_HAND
	mouse_entered.connect(queue_redraw)
	mouse_exited.connect(queue_redraw)

func update_slot(value: Dictionary, active: bool, chosen: bool, data: Dictionary) -> void:
	item = value
	selected = active
	picked = chosen
	payload = data
	tooltip_text = "空槽位" if item.is_empty() else "%s × %d" % [item.name,item.quantity]
	queue_redraw()

func _draw() -> void:
	var frame := Rect2(Vector2.ZERO,size)
	draw_rect(frame,Color("111820"))
	draw_rect(frame.grow(-2),Color("37434d"))
	draw_rect(frame.grow(-5),Color("202a32"))
	if frame.has_point(get_local_mouse_position()): draw_rect(frame.grow(-5),Color(1,1,1,0.08))
	if selected: draw_rect(frame.grow(-1),Color("ffc06b"),false,3)
	if picked: draw_rect(frame.grow(-4),Color("72cce0"),false,2)
	var font := ThemeDB.fallback_font
	if not item.is_empty():
		# Original supply-satchel icon, shared by the inventory and hotbar.
		var center := size * 0.5
		draw_rect(Rect2(center-Vector2(14,15),Vector2(28,29)),Color("1a211c"))
		draw_rect(Rect2(center-Vector2(12,17),Vector2(24,27)),Color("b4a478"))
		draw_rect(Rect2(center-Vector2(12,17),Vector2(24,6)),Color("d5c393"))
		draw_rect(Rect2(center-Vector2(8,17),Vector2(4,27)),Color("66664b"))
		draw_rect(Rect2(center+Vector2(5,-17),Vector2(4,27)),Color("66664b"))
		draw_rect(Rect2(center-Vector2(3,4),Vector2(7,8)),Color("ebe4bd"))
		var count := str(item.quantity)
		var width := font.get_string_size(count,HORIZONTAL_ALIGNMENT_LEFT,-1,16).x
		draw_string_outline(font,Vector2(size.x-width-4,size.y-5),count,HORIZONTAL_ALIGNMENT_LEFT,-1,16,4,Color("101318"))
		draw_string(font,Vector2(size.x-width-4,size.y-5),count,HORIZONTAL_ALIGNMENT_LEFT,-1,16,Color.WHITE)
	if not number.is_empty(): draw_string(font,Vector2(5,14),number,HORIZONTAL_ALIGNMENT_LEFT,-1,12,Color("bfc9d1"))

func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and event.pressed and event.button_index in [MOUSE_BUTTON_LEFT,MOUSE_BUTTON_RIGHT]:
		activated.emit(area,index,event.button_index,event.shift_pressed)
		accept_event()

func _get_drag_data(_position: Vector2) -> Variant:
	if item.is_empty() or payload.is_empty(): return null
	var preview := Label.new()
	preview.text = "%s × %d" % [item.name,payload.quantity]
	preview.add_theme_color_override("font_color",Color.WHITE)
	preview.add_theme_color_override("font_shadow_color",Color.BLACK)
	preview.add_theme_constant_override("shadow_offset_x",2)
	preview.add_theme_constant_override("shadow_offset_y",2)
	set_drag_preview(preview)
	return payload.duplicate(true)

func _can_drop_data(_position: Vector2, data: Variant) -> bool:
	return data is Dictionary and data.get("kind","") == "inventory_stack"

func _drop_data(_position: Vector2, data: Variant) -> void:
	dropped.emit(area,index,data)
