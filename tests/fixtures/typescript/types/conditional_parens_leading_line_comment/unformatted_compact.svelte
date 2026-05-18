<script lang="ts">
	// case T: leading line comment in true branch
	type T=a extends b?(// leading
	c):d;

	// case F: leading line comment in false branch
	type F=a extends b?c:(// leading
	d);
</script>
