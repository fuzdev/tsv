<!-- A run of 2+ byte-glued sibling elements followed by trailing text keeps that text on
	the run's line (folded after it); a preceding inline sibling or text contributes one
	collapsed boundary space before the run, exactly as before a single element. -->
<p><span>text1</span>   <em>a</em><em>b</em>   text2</p>
<p><em>a</em><em>b</em>   text1   text2</p>
<p><span>text1</span>   <br /><br />   text2   text3</p>
<p>text1   <em>a</em><em>b</em>   text2</p>
