extends Node2D

const TILE_SIZE := 16

# Entity type -> (fill, outline)
const ENTITY_STYLES := {
	0: { "fill": Color(0.40, 0.95, 0.45), "outline": Color(0.10, 0.45, 0.15) },  # Player
	1: { "fill": Color(0.85, 0.30, 0.30), "outline": Color(0.30, 0.05, 0.05) },  # Zombie
	2: { "fill": Color(0.95, 0.95, 0.85), "outline": Color(0.50, 0.50, 0.40) },  # Skeleton
	3: { "fill": Color(0.45, 0.80, 0.40), "outline": Color(0.15, 0.35, 0.10) },  # Goblin
	4: { "fill": Color(0.70, 0.55, 0.40), "outline": Color(0.30, 0.20, 0.10) },  # Rat
}

const PLAYER_RADIUS := 6.5
const ENEMY_RADIUS := 5.5
const PLAYER_HALO := Color(1.0, 1.0, 0.6, 0.25)

# Snapshot of last-seen entities (each: { x:int, y:int, type:int, is_player:bool })
var _entities := []

func sync(client: Node):
	_entities.clear()
	var count = client.get_entity_count()
	for i in range(count):
		var e = client.get_entity(i)
		if e.is_empty():
			continue
		_entities.append({
			"x": int(e.get("pos_x", 0)),
			"y": int(e.get("pos_y", 0)),
			"type": int(e.get("entity_type", 0)),
			"is_player": bool(e.get("is_player", false)),
		})
	queue_redraw()

func _draw():
	for e in _entities:
		var center := Vector2(
			e.x * TILE_SIZE + TILE_SIZE * 0.5,
			e.y * TILE_SIZE + TILE_SIZE * 0.5,
		)
		var style: Dictionary = ENTITY_STYLES.get(e.type, {
			"fill": Color.WHITE,
			"outline": Color.BLACK,
		})
		var radius: float = PLAYER_RADIUS if e.is_player else ENEMY_RADIUS

		if e.is_player:
			# Soft halo behind the player so they pop against any tile.
			draw_circle(center, radius + 2.5, PLAYER_HALO)

		draw_circle(center, radius, style["fill"])
		draw_arc(center, radius, 0.0, TAU, 24, style["outline"], 1.5)
