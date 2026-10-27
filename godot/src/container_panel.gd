extends PanelContainer
signal transfer_requested(request: Dictionary)
signal move_requested(revision: int, source: int, target: int, quantity: int)
signal quick_move_requested(revision: int, slot: int)
signal close_requested
const Slot = preload("res://src/item_slot.gd")
const Palette = preload("res://src/interface_theme.gd")
var player_slots: Array = []
var chest_slots: Array = []
var chest_section: VBoxContainer
var details: VBoxContainer
var feedback: Label
var heading: Label
var view: Dictionary = {}
var selected: Dictionary = {}
var snapshot: Dictionary = {}

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_CENTER)
	var column := VBoxContainer.new()
	column.add_theme_constant_override("separation",12)
	add_child(column)
	var top := HBoxContainer.new()
	column.add_child(top)
	heading = _label(top,"物品栏",23)
	heading.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	var close := Button.new()
	close.text = "关闭 / Esc"
	close.pressed.connect(func(): close_requested.emit())
	top.add_child(close)
	var body := HBoxContainer.new()
	body.add_theme_constant_override("separation",24)
	column.add_child(body)
	var grids := VBoxContainer.new()
	grids.add_theme_constant_override("separation",10)
	body.add_child(grids)
	chest_section = VBoxContainer.new()
	grids.add_child(chest_section)
	_label(chest_section,"箱子 · Shift 点击快速取出",15)
	var chest_grid := GridContainer.new()
	chest_grid.columns = 8
	chest_section.add_child(chest_grid)
	for i in range(24): chest_slots.append(_slot(chest_grid,"chest",i))
	_label(grids,"背包",16)
	var bag_grid := GridContainer.new()
	bag_grid.columns = 9
	grids.add_child(bag_grid)
	player_slots.resize(36)
	for i in range(9,36): player_slots[i] = _slot(bag_grid,"player",i)
	_label(grids,"快捷栏",16)
	var bar := HBoxContainer.new()
	grids.add_child(bar)
	for i in range(9):
		player_slots[i] = _slot(bar,"player",i)
		player_slots[i].number = str(i+1)
	details = VBoxContainer.new()
	details.custom_minimum_size.x = 225
	details.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	body.add_child(details)
	feedback = _label(column,"",14)
	feedback.modulate = Palette.ACCENT
	feedback.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	hide()

func _label(parent: Node, text: String, size: int) -> Label:
	var label := Label.new()
	label.text = text
	label.add_theme_font_size_override("font_size",size)
	parent.add_child(label)
	return label

func _slot(parent: Node, area: String, index: int) -> Control:
	var slot := Slot.new()
	slot.area = area
	slot.index = index
	parent.add_child(slot)
	slot.activated.connect(_activate)
	slot.dropped.connect(_drop)
	return slot

func clear_selection() -> void:
	selected = {}

func update_view(state: Dictionary, with_chest: bool) -> void:
	snapshot = state
	view = state.container if with_chest else {}
	chest_section.visible = with_chest
	heading.text = "箱子 / %d 类物品" % view.get("items",[]).size() if with_chest else "物品栏 / 36 格"
	var height := 650 if with_chest else 450
	offset_left = -425
	offset_right = 425
	offset_top = -height/2
	offset_bottom = height/2
	if not selected.is_empty() and (selected.inventory_revision != state.inventory_revision or selected.revision != view.get("revision",-1)): clear_selection()
	feedback.text = state.container_error if not state.container_error.is_empty() else "拖动或左键移动 · 右键拆半 / 放 1 个 · Shift 快速转移"
	if not selected.is_empty(): feedback.text = "已选中 %d 个 · 点击目标槽位（Esc 取消）" % selected.quantity
	for i in range(36):
		player_slots[i].update_slot(state.inventory_slots[i],i == state.selected_slot,selected.get("area","") == "player" and selected.get("slot",-1) == i,_payload("player",i))
	for i in range(24):
		chest_slots[i].update_slot(_item("chest",i),false,selected.get("area","") == "chest" and selected.get("slot",-1) == i,_payload("chest",i))

func _item(area: String, index: int) -> Dictionary:
	var items: Array = snapshot.get("inventory_slots",[]) if area == "player" else view.get("items",[])
	return items[index] if index >= 0 and index < items.size() else {}

func _payload(area: String, index: int) -> Dictionary:
	var item := _item(area,index)
	if item.is_empty(): return {}
	return {"kind":"inventory_stack","area":area,"slot":index,"quantity":mini(int(item.quantity),int(snapshot.stack_limit)),"inventory_revision":snapshot.inventory_revision,"revision":view.get("revision",-1),"position":view.get("position",Vector3i.ZERO)}

func _activate(area: String, index: int, mouse_button: int, quick: bool) -> void:
	var source := _payload(area,index)
	if quick:
		clear_selection()
		if source.is_empty(): return
		if not view.is_empty(): _transfer(source,-1)
		elif area == "player": quick_move_requested.emit(snapshot.inventory_revision,index)
		return
	if selected.is_empty():
		if source.is_empty(): return
		selected = source
		if mouse_button == MOUSE_BUTTON_RIGHT: selected.quantity = ceili(float(source.quantity)/2.0)
	else:
		var moving := selected.duplicate(true)
		if mouse_button == MOUSE_BUTTON_RIGHT: moving.quantity = 1
		_drop(area,index,moving)

func _drop(area: String, index: int, source: Dictionary) -> void:
	clear_selection()
	if source.area == area and source.slot == index: return
	if source.area == "player" and area == "player":
		move_requested.emit(source.inventory_revision,source.slot,index,source.quantity)
	elif not view.is_empty() and source.area != area:
		_transfer(source,index if area == "player" else -1)

func _transfer(source: Dictionary, target: int) -> void:
	transfer_requested.emit({"position":source.position,"revision":source.revision,"inventory_revision":source.inventory_revision,"slot":source.slot,"quantity":source.quantity,"deposit":source.area == "player","target":target})
