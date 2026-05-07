extends TileMap

var tile_size = Vector2i(16, 16)
var tile_cache = {}

func sync(client: RefCounted):
	var count = client.get_visible_tile_count()
	for i in range(count):
		var t = client.get_visible_tile(i)
		if t.is_empty():
			continue
		var pos = Vector2i(t.get("pos_x", 0), t.get("pos_y", 0))
		var tile_type = t.get("tile_type", 0)
		set_cell(0, pos, tile_type, Vector2i(0, 0))
