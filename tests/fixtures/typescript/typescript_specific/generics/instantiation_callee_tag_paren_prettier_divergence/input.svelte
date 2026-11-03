<script lang="ts">
	// an instantiation as a call's callee or a template's tag keeps its pair
	(f<T>)();
	(a.b<T>)(c);
	(f<T>)`x`;
	(a.b<T>)`x`;
	x = (f<T>)(c).d;
	x = (f<T>)`x`.y;
	new (f<T>)`x`();
	// a member chain over the call breaks with the pair kept on its base
	x = (aaaaaaaaaaaaaaaaaaa<T>)(bbbbbbbbbbbbbbbbbb)
		.ccccccccccccccccccc()
		.ddddddddddddddddddd()
		.eeeeeeeeeeeeee();
	// an optional call reads the instantiation as its callee bare, so the pair strips
	f<T>?.();
</script>

{(f<T>)()}
{(f<T>)`x`}
