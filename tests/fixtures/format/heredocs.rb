z = <<-FOO
  hi #{n}
FOO

def build
  sql = <<~SQL
    SELECT *
      FROM users
     WHERE id = #{id}
  SQL
  sql
end

pair = foo(<<~A, <<~B)
  first body
A
  second body
B

plain = <<~'RAW'
  no #{interpolation} here
RAW

empty = <<~NONE
NONE

klass.class_eval <<-METHOD, __FILE__, __LINE__ + 1
  def #{name}(*args, &block)
    invoke(*args, &block)
  end
METHOD

class Mapper
  def deprecated
    warn(<<~MSG.squish)
      Something is deprecated
        and this line is deeper

      #{name} is a hole
    MSG
  end

  def dash
    sql = <<-SQL
      keeps its whitespace
    SQL
    sql
  end
end

f = <<`CMD`
  echo hi
CMD
f = <<-`CMD`
  echo hi
CMD
f = <<~`CMD`
  echo hi
CMD
g = <<~`CMD`
  echo #{a}
CMD

execute <<~A if foo(<<~B)
  body_a
A
  body_b
B

execute <<~A if foo(<<~B, <<~C)
  body_a
A
  body_b
B
  body_c
C

execute <<~SQL if DB.query_single(<<~SQL).length > 0
  ALTER TABLE email_tokens ALTER COLUMN token DROP NOT NULL
SQL
  SELECT 1
SQL
