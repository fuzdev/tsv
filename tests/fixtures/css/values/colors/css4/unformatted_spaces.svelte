<style>
	div {
		/* RGB with percentages */
		color: rgb(100%  0%  0%);
		background: rgb(0%   100%   0%);
		/* RGB with none keyword */
		color: rgb(255   0   none);
		background: rgb(none  255  128);
		/* RGB with percentage alpha */
		color: rgb(255   0   0   /   50%);
		background: rgba(0,255,0,75%);
		/* HSL with angle units */
		color: hsl(120deg,  75%,  25%);
		background: hsl(180deg   50%   50%);
		/* HSL with none keyword */
		color: hsl(none   50%   50%);
		background: hsl(180  none  50%);
		/* HSL with percentage alpha */
		color: hsl(0   100%   50%   /   50%);
		background: hsla(120,100%,50%,75%);
		/* Mixed: RGB percentages + none + percentage alpha */
		color: rgb(100%   0%   none   /   50%);
		/* Mixed: HSL with deg + none + percentage alpha */
		color: hsl(120deg  none  50%  /  75%);
	}
</style>
