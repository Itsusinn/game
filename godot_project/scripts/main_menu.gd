extends Control

@onready var addr_input: LineEdit = $Panel/AddrInput
@onready var port_input: SpinBox = $Panel/PortInput
@onready var name_input: LineEdit = $Panel/NameInput
@onready var status_label: Label = $Panel/StatusLabel
@onready var connect_btn: Button = $Panel/ConnectBtn

func _ready():
	addr_input.text = Global.server_address
	port_input.value = Global.server_port
	name_input.text = Global.player_name

func _on_connect_pressed():
	Global.server_address = addr_input.text
	Global.server_port = int(port_input.value)
	Global.player_name = name_input.text

	status_label.text = "Connecting..."
	connect_btn.disabled = true

	var cdda_class = ClassDB.class_exists("CddaClient")
	if not cdda_class:
		status_label.text = "ERROR: GDExtension not loaded"
		connect_btn.disabled = false
		return

	get_tree().change_scene_to_file("res://scenes/game.tscn")
