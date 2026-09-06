<script lang="ts">
	// `shouldBreakAfterOperator` asks the poorly-breakable question of the value UNDER its
	// wrappers - a unary operator, `await`, a `!` - so a wrapped chain breaks after the `=`
	// and stays whole, exactly as the bare chain does.

	// 100 - fits
	const aaaa=!!this.cccc.dddd()?.eeeeeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.kkkk?.llll?.mmmm?.nnnn;

	// 101 - break after `=`, the chain whole
	const aaaaa =!!this.cccc.dddd()?.eeeeeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.kkkk?.llll?.mmmm?.nnnn;
	const bbbb =-this.cccc.dddd()?.eeeeeeeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.kkkk?.llll?.mmmm?.nnnn;
	const cccc =typeof this.cccc.dddd()?.eeeeeeeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.kkkk?.llll?.mmmm;

	async function fn() {
		const dddd =await this.cccc.dddd()?.eeeeeeeeeee?.ffff?.gggg?.hhhh?.iiii?.jjjj?.kkkk?.llll?.mmmm;
	}
</script>
