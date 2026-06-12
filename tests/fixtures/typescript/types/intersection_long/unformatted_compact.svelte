<script lang="ts">
	// 100 chars - stays inline (at boundary)
	type Intersection100=Aaaaaaaaaaaaaaaaaaaa&Bbbbbbbbbbbbbbbbbbb&Ccccccccccccccccccc&Ddddddd;

	// 101 chars - wraps (exceeds print_width)
	type Intersection101=Aaaaaaaaaaaaaaaaaaaa&Bbbbbbbbbbbbbbbbbbb&Ccccccccccccccccccc&Dddddddd;
</script>
