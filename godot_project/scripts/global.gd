extends Node

var server_address := "127.0.0.1"
var server_port := 9876
var player_name := "Player"

func get_addr() -> String:
	return "%s:%d" % [server_address, server_port]
