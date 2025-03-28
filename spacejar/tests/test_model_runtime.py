from model_runtime import ExampleClass, PythonClass


def test_python_class() -> None:
    py_class = PythonClass(value=10)
    assert py_class.value == 10


def test_example_class() -> None:
    example = ExampleClass(value=11)
    assert example.value == 11


def test_doc() -> None:
    import model_runtime

    assert model_runtime.__doc__ == "An example module implemented in Rust using PyO3."
