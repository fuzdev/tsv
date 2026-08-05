<script lang="ts">
	// source-multiline object head before an object-body arrow: all args break out
	obj.aa(x).bb(
		{
			h: 1
		},
		(l) => ({ v: l })
	);

	// flat head keeps the args inline
	obj.aa(x).bb({ h: 1 }, (l) => ({ v: l }));
</script>
