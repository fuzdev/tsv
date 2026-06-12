<script>
	// 100 chars - stays inline
	const a=(COND_AAAAAAAAAAAAAAAAAAA&&Fn_bbbbbbb.parse(Fn_ccc.parse(valueeeeeeeeeeeeeeeee)))||fallback;

	// 101 chars - breaks at || not inside call
	const b=(COND_AAAAAAAAAAAAAAAAAAA&&Fn_bbbbbbbb.parse(Fn_ccc.parse(valueeeeeeeeeeeeeeeee)))||fallback;

	// Very long - breaks at && and ||
	const c=(VERY_LONG_CONDITION_VARIABLE_NAMEEEEEEE&&Fn_bbbbbbb.parse(Fn_ccc.parse(valueeeeeeeeeeeeeeeee)))||fallback;
</script>
