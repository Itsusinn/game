extends Node2D

const TILE_SIZE := 16

# Tile type → color mapping (CDDA-style)
const TILE_COLORS := {
	0: Color(0.4, 0.4, 0.4),  # Floor
	1: Color(0.6, 0.5, 0.3),  # Wall
	2: Color(0.3, 0.3, 0.5),  # Door open
	3: Color(0.5, 0.3, 0.2),  # Door closed
	4: Color(0.2, 0.3, 0.6),  # Water
	5: Color(0.5, 0.5, 0.5),  # Stairs up
	6: Color(0.3, 0.3, 0.3),  # Stairs down
}

var _tiles := []

func sync(client: Node):
	_tiles.clear()
	var count = client.get_visible_tile_count()
	if count == 0:
		queue_redraw()
		return
	for i in range(count):
		var t = client.get_visible_tile(i)
		if t.is_empty():
			continue
		var pos_x = t.get("pos_x", 0)
		var pos_y = t.get("pos_y", 0)
		var tile_type = t.get("tile_type", 0)
		var bg_color = t.get("bg_color", 0)
		_tiles.append({
			x = pos_x, y = pos_y,
			tile_type = tile_type,
			bg = bg_color
		})
	queue_redraw()

func _draw():
	for tile in _tiles:
		var color: Color
		if tile.bg != 0:
			color = Color(
				((tile.bg >> 16) & 0xFF) / 255.0,
				((tile.bg >> 8) & 0xFF) / 255.0,
				(tile.bg & 0xFF) / 255.0
			)
		else:
			color = TILE_COLORS.get(tile.tile_type, Color.WHITE)
		var rect = Rect2(tile.x * TILE_SIZE, tile.y * TILE_SIZE, TILE_SIZE, TILE_SIZE)
		draw_rect(rect, color, true)
