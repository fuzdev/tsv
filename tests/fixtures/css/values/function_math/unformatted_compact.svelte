<style>
	div {
		/* Comparison functions (CSS Values 4) */
		width:min(100px,50vw,80%);
		height:max(200px,30vh);
		font-size:clamp(12px,2vw,24px);
		/* Stepped value functions (CSS Values 4) */
		--steps:round(10.7px);
		--remainder:mod(18px,5px);
		--rem-val:rem(18px,5px);
		/* Sign-related functions (CSS Values 4) */
		--abs:abs(-10px);
		--sign:sign(-5);
		/* Trigonometric functions (CSS Values 4) */
		transform:rotate(sin(45deg));
		--angle:cos(30deg);
		--tangent:tan(60deg);
		--asin:asin(0.5);
		--acos:acos(0.5);
		--atan:atan(1);
		--atan2:atan2(1,1);
		/* Exponential functions (CSS Values 5) */
		opacity:pow(0.5,2);
		--sqrt:sqrt(144);
		--hypot:hypot(3,4);
		--exp:exp(2);
		--log:log(100);
	}
</style>
