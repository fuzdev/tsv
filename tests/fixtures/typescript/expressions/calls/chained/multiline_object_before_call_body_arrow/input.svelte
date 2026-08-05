<script lang="ts">
	// source-multiline object head before a call-body arrow: all args break out
	obj.aa(x).bb(
		{
			h: 1
		},
		(l) => g(l)
	);

	// flat head keeps the args inline
	obj.aa(x).bb({ h: 1 }, (l) => g(l));
</script>
