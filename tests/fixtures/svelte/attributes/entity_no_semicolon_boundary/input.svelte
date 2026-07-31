<!--
	A named reference that drops its semicolon decodes inside an attribute value unless the
	character after it is `=` or an ASCII alphanumeric. A non-ASCII letter or digit is not
	ASCII alphanumeric, so such a reference still decodes. Text content asks no such
	question — a reference there always decodes.
-->
<div data-attr="&AMP=1 &AMPersand &AMP1"></div>
<div data-attr="&AMP中 &COPYé &nbsp１"></div>
<div data-attr="&AMP; &AMP- &AMP"></div>
<p>&AMPersand &AMP中</p>
