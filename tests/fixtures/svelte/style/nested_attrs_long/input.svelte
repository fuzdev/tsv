<!-- exactly 100 cols: the `>` hugs the last attribute -->
<div>
	<style lang="css" data-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa="value1" data-bbbbbbbbbbbbbb="value2">
		div {
			color: red;
		}
	</style>
</div>

<!-- 101 cols: the attributes wrap and the `>` dedents to its own line -->
<div>
	<style
		lang="css"
		data-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa="value1"
		data-bbbbbbbbbbbbbbb="value2"
	>
		div {
			color: red;
		}
	</style>
</div>
