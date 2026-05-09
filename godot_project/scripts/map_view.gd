extends Node2D

const TILE_SIZE := 16

# Visible-tile palette (bright, full color).
const TILE_COLORS := {
	0: Color(0.18, 0.18, 0.22),   # Floor — dark slate
	1: Color(0.55, 0.45, 0.30),   # Wall — warm stone
	2: Color(0.45, 0.55, 0.30),   # Door open — green
	3: Color(0.55, 0.30, 0.20),   # Door closed — red-brown
	4: Color(0.18, 0.35, 0.65),   # Water — blue
	5: Color(0.85, 0.80, 0.40),   # Stairs up — yellow
	6: Color(0.55, 0.40, 0.75),   # Stairs down — purple
}

const FLOOR_GRID := Color(0.10, 0.10, 0.12)
const WALL_OUTLINE := Color(0.78, 0.65, 0.42)
const FOG_DARKEN := 0.65       # how much to dim explored-but-not-visible tiles
const FOG_DESATURATE := 0.55   # blend factor toward gray for fogged tiles

# Cached tiles ever seen, keyed by Vector2i(pos_x, pos_y).
# Each entry: { tile_type: int }
var _explored_cache := {}

# Set of currently-visible Vector2i positions (refreshed each sync).
var _visible_set := {}

func sync(client: Node):
	_visible_set.clear()
	var count = client.get_visible_tile_count()
	for i in range(count):
		var t = client.get_visible_tile(i)
		if t.is_empty():
			continue
		var pos := Vector2i(int(t.get("pos_x", 0)), int(t.get("pos_y", 0)))
		var tile_type: int = int(t.get("tile_type", 0))
		_visible_set[pos] = true
		_explored_cache[pos] = tile_type
	queue_redraw()

func _draw():
	for pos in _explored_cache:
		var tile_type: int = _explored_cache[pos]
		var visible: bool = _visible_set.has(pos)
		_draw_tile(pos, tile_type, visible)

func _draw_tile(pos: Vector2i, tile_type: int, visible: bool) -> void:
	var base: Color = TILE_COLORS.get(tile_type, Color.WHITE)
	var color: Color = base if visible else _fogged(base)
	var px := float(pos.x * TILE_SIZE)
	var py := float(pos.y * TILE_SIZE)
	var rect := Rect2(px, py, TILE_SIZE, TILE_SIZE)
	draw_rect(rect, color, true)

	match tile_type:
		0:
			# Floor: subtle grid line
			var grid := FLOOR_GRID if visible else _fogged(FLOOR_GRID)
			draw_rect(rect, grid, false, 1.0)
		1:
			# Wall: highlight top & left edges for a beveled look
			var hi := WALL_OUTLINE if visible else _fogged(WALL_OUTLINE)
			draw_line(Vector2(px, py), Vector2(px + TILE_SIZE, py), hi, 1.0)
			draw_line(Vector2(px, py), Vector2(px, py + TILE_SIZE), hi, 1.0)
		2, 3:
			# Door: draw a frame inset
			var inset := rect.grow(-2.0)
			var frame := Color(0.85, 0.75, 0.50)
			if not visible:
				frame = _fogged(frame)
			draw_rect(inset, frame, false, 1.0)
		4:
			# Water: two small ripple dots
			var dot := Color(0.65, 0.85, 1.0)
			if not visible:
				dot = _fogged(dot)
			draw_circle(Vector2(px + 5, py + 7), 1.5, dot)
			draw_circle(Vector2(px + 11, py + 11), 1.5, dot)
		5:
			# Stairs up: upward triangle
			var tri_up := PackedVector2Array([
				Vector2(px + TILE_SIZE * 0.5, py + 3),
				Vector2(px + 3, py + TILE_SIZE - 3),
				Vector2(px + TILE_SIZE - 3, py + TILE_SIZE - 3),
			])
			var c := Color(0.20, 0.20, 0.10)
			if not visible:
				c = _fogged(c)
			draw_colored_polygon(tri_up, c)
		6:
			# Stairs down: downward triangle
			var tri_dn := PackedVector2Array([
				Vector2(px + 3, py + 3),
				Vector2(px + TILE_SIZE - 3, py + 3),
				Vector2(px + TILE_SIZE * 0.5, py + TILE_SIZE - 3),
			])
			var c2 := Color(0.20, 0.10, 0.25)
			if not visible:
				c2 = _fogged(c2)
			draw_colored_polygon(tri_dn, c2)
		_:
			pass

func _fogged(color: Color) -> Color:
	# Darken and desaturate so explored-but-not-visible tiles read as memory.
	var dim := color * (1.0 - FOG_DARKEN)
	dim.a = color.a
	var gray_v := (dim.r + dim.g + dim.b) / 3.0
	var gray := Color(gray_v, gray_v, gray_v, dim.a)
	return dim.lerp(gray, FOG_DESATURATE)
