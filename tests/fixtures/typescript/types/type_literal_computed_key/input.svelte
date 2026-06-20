<script lang="ts">
	type Computed = {
		[a]: A;
		[b]?: B;
		[c]?;
		readonly [d]: D;
		[e.f]: E;
		['g']: G;
		[h](): H;
	};
</script>
