<script lang="ts">
	// Cast argument of a postfix update keeps parens — bare a as T++ binds ++ to T
	(a as T)++;
	// Prefix update over a cast keeps parens too
	++(a as T);
	// Redundant parens around a plain reference are stripped
	a.b++;
</script>
