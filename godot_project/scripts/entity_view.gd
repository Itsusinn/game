extends Node2D

const TILE_SIZE := 16

# Entity type → color mapping
const ENTITY_COLORS := {
	0: Color(0.2, 0.8, 0.2),  # Player
	1: Color(0.8, 0.2, 0.2),  # Zombie
	2: Color(0.9, 0.9, 0.8),  # Skeleton
	3: Color(0.3, 0.7, 0.3),  # Goblin
	4: Color(0.6, 0.6, 0.6),  # Rat
}

var entity_sprites := {}

func sync(client: Node):
	var seen_ids = {}
	var count = client.get_entity_count()
	for i in range(count):
		var e = client.get_entity(i)
		if e.is_empty():
			continue
		var id = e.get("id", -1)
		seen_ids[id] = e

	# Add new / update existing
	for id in seen_ids:
		var e = seen_ids[id]
		var px = e.get("pos_x", 0) * TILE_SIZE
		var py = e.get("pos_y", 0) * TILE_SIZE
		var etype = e.get("entity_type", 0)
		var is_player = e.get("is_player", false)

		if entity_sprites.has(id):
			var sprite = entity_sprites[id]
			sprite.position = Vector2(px, py)
		else:
			var rect = ColorRect.new()
			rect.size = Vector2(TILE_SIZE, TILE_SIZE)
			rect.position = Vector2(px, py)
			var color = ENTITY_COLORS.get(etype, Color.WHITE)
			if is_player:
				color = Color(0.0, 1.0, 0.0)  # Bright green for player
			rect.color = color
			add_child(rect)
			entity_sprites[id] = rect

	# Remove gone entities
	var to_remove = []
	for id in entity_sprites:
		if not seen_ids.has(id):
			to_remove.append(id)
	for id in to_remove:
		entity_sprites[id].queue_free()
		entity_sprites.erase(id)
