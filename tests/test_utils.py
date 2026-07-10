from datetime import date

from utils import data_pack


def test_data_pack_converts_to_roc_year():
    result = data_pack(date(2024, 3, 5))
    assert result["date"] == "1130305"


def test_data_pack_pads_month_and_day():
    result = data_pack(date(2026, 1, 9))
    assert result["date"] == "1150109"


def test_data_pack_year_boundary():
    result = data_pack(date(1912, 1, 1))
    assert result["date"] == "0010101"


def test_data_pack_includes_work_content():
    result = data_pack(date(2024, 3, 5))
    assert result["work"] == "協助教授計畫"
