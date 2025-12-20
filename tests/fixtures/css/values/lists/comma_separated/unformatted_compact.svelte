<style>
	div {
		/* Font-family lists */
		font-family: 'Helvetica Neue',Arial,sans-serif;
		font: 'Times New Roman',Times,serif;
		/* Multiple backgrounds */
		background: url(1.png),url(2.png);
		background-image: url(top.png),url(bottom.png),linear-gradient(to bottom,#fff,#000);
		/* Transition properties */
		transition:opacity 0.3s ease,transform 0.3s ease-out;
		-webkit-transition:all 0.2s,color 0.5s linear;
		/* Animation properties */
		animation:fade 1s ease-in,slide 0.5s ease-out,rotate 2s linear;
		animation-name: spin,bounce,wobble;
		animation-duration: 1s,2s,1.5s;
		/* Text-shadow - multiple shadows */
		text-shadow:1px 1px 2px #000,0 0 5px rgba(255,255,255,0.5),-1px -1px 3px blue;
		/* Box-shadow - space-separated values */
		box-shadow:2px 2px 4px rgba(0,0,0,0.3),0 0 10px rgba(255,255,255,0.2);
		/* Function with trailing values (triggers multiline) */
		border:rgb(255,0,0) solid 2px,rgb(0,255,0) dashed 1px;
		/* One bad apple - mixed atomic and space-separated */
		prop1:atomic,value1 value2;
		/* Pure atomic values - stay inline */
		prop2: atomic1,atomic2;
		/* Custom properties - stay inline despite structure */
		--my-transition: opacity 0.3s ease,transform 0.3s ease-out;
		--spacing: 10px 20px,5px 15px;
		/* Mixed values */
		outline:url(pattern.png) repeat,linear-gradient(to right,red,blue) no-repeat center / cover;
	}
</style>
