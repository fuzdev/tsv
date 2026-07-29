<script>
	let a = 1;
</script>

<!-- Glued block siblings inside a range keep the author's bytes -->
<!-- prettier-ignore-start -->
<p>block1</p><p>block2</p>
<!-- prettier-ignore-end -->

<!-- Glued inline siblings: the glue is render-significant; both tools keep it -->
<!-- prettier-ignore-start -->
<span>inline1</span><span>inline2</span>
<!-- prettier-ignore-end -->

<!-- Content glued to the markers themselves -->
<!-- prettier-ignore-start --><div      data-attr="value1"   ></div><!-- prettier-ignore-end -->
