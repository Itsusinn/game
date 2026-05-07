extends Control

func update(client: RefCounted):
	if has_node("HP"):
		$HP.text = "HP: %d/%d" % [client.get_player_hp(), client.get_player_max_hp()]
	if has_node("Stamina"):
		$Stamina.text = "STA: %d" % client.get_player_stamina()
	if has_node("Log"):
		_update_log(client)

func _update_log(client: RefCounted):
	var count = client.get_log_count()
	var text = ""
	for i in range(count):
		var entry = client.get_log_entry(i)
		text += entry.get("text", "") + "\n"
	if has_node("Log"):
		$Log.text = text
