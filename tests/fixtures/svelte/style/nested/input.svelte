<div>
	<style>
		div {
			color: red;
		}
		p {
			margin: 0;
		}
	</style>
</div>

{#if condition}
	<style>
		.conditional {
			display: block;
		}
	</style>
{/if}

<!-- with attributes: the `>` hugs the last attribute when they fit -->
<div>
	<style lang="css" data-attr="value">
		div {
			color: blue;
		}
	</style>
</div>

<div><style data-attr="value"></style></div>

<!-- blank lines between declarations and between rules are kept -->
<div>
	<style>
		.class1 {
			color: red;

			margin: 0;
		}

		.class2 {
			color: blue;
		}
	</style>
</div>
