<script lang="ts">
	// 100 chars exactly - stays inline (at boundary)
	switch  (  condA   +   condB   +   condC   +   condD   +   condE   +   condF   +   condG   +   condH   +   condI   +   condJJJJJJJJJJJ  )   {
	}

	// 101 chars - wraps (exceeds print_width)
	switch  (
		condA   +
		condB   +
		condC   +
		condD   +
		condE   +
		condF   +
		condG   +
		condH   +
		condI   +
		condJJJJJJJJJJJJ
	)   {
	}

	// 101 chars with cases - wraps
	switch  (
		condA   +
		condB   +
		condC   +
		condD   +
		condE   +
		condF   +
		condG   +
		condH   +
		condI   +
		condJJJJJJJJJJJJ
	)   {
		case   1  :
			break  ;
		case   2  :
			x   =   y  ;
			break  ;
		default  :
			break  ;
	}
</script>
