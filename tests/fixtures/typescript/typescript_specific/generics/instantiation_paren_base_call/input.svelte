<script lang="ts">
	// an instantiation over a parenthesized base is the chain's own, so its list
	// joins the call or the tag
	f<T>();
	f<T>`x`;
	a.b<T>(c);
</script>

{f<T>()}
{f<T>`x`}
