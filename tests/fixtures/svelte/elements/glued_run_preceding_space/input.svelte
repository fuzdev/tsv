<!-- A space between an inline sibling and a following run of 2+ glued sibling
	elements is inter-sibling whitespace (render-significant, collapses to one
	space) and stays a single space, exactly as before a single sibling. -->
<p><span>text1</span> <br /><br /></p>
<p><span>text2</span> <em>a</em><em>b</em></p>
<p><span>text3</span> <br /></p>
