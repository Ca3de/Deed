from setuptools import setup, find_packages

with open("README.md", "r", encoding="utf-8") as fh:
    long_description = fh.read()

setup(
    name="deed-db",
    version="0.1.0",
    author="Deed Project",
    description="A biologically-inspired hybrid database unifying relational and graph models",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/yourusername/deed",
    packages=find_packages(),
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "Topic :: Database :: Database Engines/Servers",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
    ],
    python_requires=">=3.8",
    install_requires=[
        # Core has zero dependencies!
    ],
    extras_require={
        "dev": [
            "pytest>=6.0",
            "numpy>=1.20.0",
            "networkx>=2.6",
            "matplotlib>=3.4",
        ],
    },
)
