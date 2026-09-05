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

  def overlong_arguments(name)
    super(
      name.to_s,
      description,
      long_description,
      wrap_long_description,
      usage,
      options_or_default,
      options_relation_or_default
    )
  end
end
