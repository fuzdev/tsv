<style>
	/* :is() forgiving list keeps a bare number as an Nth node (not dropped) */
	:is(.a,123){color:red}

	/* :not() list: a bare An+B is an Nth node */
	:not(2n){color:blue}

	/* :has() relative list: a bare number is an Nth node */
	:has(123){color:green}
</style>
