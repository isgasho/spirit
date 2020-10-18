(function() {var implementors = {};
implementors["futures"] = [];
implementors["hyper"] = [{"text":"impl Sink for Sender","synthetic":false,"types":[]}];
implementors["spirit_tokio"] = [{"text":"impl&lt;A, B&gt; Sink for Either&lt;A, B&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;A: Sink,<br>&nbsp;&nbsp;&nbsp;&nbsp;B: Sink&lt;SinkError = A::SinkError, SinkItem = A::SinkItem&gt;,&nbsp;</span>","synthetic":false,"types":[]}];
implementors["tokio_sync"] = [{"text":"impl&lt;T&gt; Sink for Sender&lt;T&gt;","synthetic":false,"types":[]},{"text":"impl&lt;T&gt; Sink for UnboundedSender&lt;T&gt;","synthetic":false,"types":[]},{"text":"impl&lt;T&gt; Sink for Sender&lt;T&gt;","synthetic":false,"types":[]}];
implementors["tokio_udp"] = [{"text":"impl&lt;C:&nbsp;Encoder&gt; Sink for UdpFramed&lt;C&gt;","synthetic":false,"types":[]}];
implementors["tokio_uds"] = [{"text":"impl&lt;A:&nbsp;AsRef&lt;Path&gt;, C:&nbsp;Encoder&gt; Sink for UnixDatagramFramed&lt;A, C&gt;","synthetic":false,"types":[]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()