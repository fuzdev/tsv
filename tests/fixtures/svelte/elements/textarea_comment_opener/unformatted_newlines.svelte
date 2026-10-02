<!-- RCDATA: an unterminated `<!--` opening the content is literal text, not a comment -->
<textarea><!-- text1</textarea>



<!-- nested in an element -->
<div><textarea><!-- text2</textarea></div>



<!-- behind leading whitespace -->
<textarea> <!-- text3</textarea>



<!-- a terminated one is literal text too -->
<textarea><!-- text4 --></textarea>



<!-- glued, with no content after it -->
<textarea><!--</textarea>
