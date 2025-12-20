<style>
	div {
		/* Resolution selection */
		background: image-set(  'image.png'  1x  ,  'image@2x.png'  2x  );
		/* Image blending */
		background: cross-fade(  url(a.png)  ,  url(b.png)  ,  50%  );
		/* Element as image */
		background: element(  #myElement  );
	}
</style>
