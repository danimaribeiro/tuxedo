class Child < Parent
  def initialize(a, b)
    super(a, b)
  end

  def initialize_without_parens(a, b)
    super a, b
  end

  def full_forward
    super
  end

  def with_block(&block)
    super(a, &block)
  end
end
