module Thor
  Correctable = if defined?(DidYouMean::SpellChecker) && defined?(DidYouMean::Correctable) && defined?(DidYouMean::Correctable2) # rubocop:disable Naming/ConstantName
    1
  end
  foo(aaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccccc, ddddddddddddddddddd) # rubocop:disable Style/Foo
  foo(aaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccccccccccc, ddddddddddddddddddd)
end

class Concern
  def check
    included.class_methods { def foo; @foo << :included; end } # rubocop:disable Lint/NestedMethodDefinition
    included.class_methods { def foo; @foo << :included; end }
  end
end
