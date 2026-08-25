<!-- width is not the axis: at 100 the hugged line would fit exactly, and the block still takes
     its own line -->
<div>
	text1 text2 <div>eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee</div>
</div>

<!-- at 101 the hugged line overflows; the block collapses inline on its fresh line (89 chars)
     rather than opening block-style with its head left on the text line -->
<div>
	text1 text2 <div>eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee</div>
</div>
