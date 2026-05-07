extends Node2D

var entity_sprites = {}

func sync(client: RefCounted):
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
		var pos = Vector2(e.get("pos_x", 0) * 16, e.get("pos_y", 0) * 16)
		if entity_sprites.has(id):
			entity_sprites[id].position = pos
		else:
			var sprite = Sprite2D.new()
			add_child(sprite)
			sprite.position = pos
			entity_sprites[id] = sprite

	# Remove gone entities
	var to_remove = []
	for id in entity_sprites:
		if not seen_ids.has(id):
			to_remove.append(id)
	for id in to_remove:
		entity_sprites[id].queue_free()
		entity_sprites.erase(id)
