#!/usr/bin/env python3
"""
生成大量测试数据用于DataFlare性能测试
"""

import csv
import random
import string
from datetime import datetime, timedelta
import os

def generate_random_name():
    """生成随机姓名"""
    first_names = [
        "张", "李", "王", "刘", "陈", "杨", "赵", "黄", "周", "吴",
        "徐", "孙", "胡", "朱", "高", "林", "何", "郭", "马", "罗",
        "梁", "宋", "郑", "谢", "韩", "唐", "冯", "于", "董", "萧"
    ]
    
    second_names = [
        "伟", "芳", "娜", "秀英", "敏", "静", "丽", "强", "磊", "军",
        "洋", "勇", "艳", "杰", "娟", "涛", "明", "超", "秀兰", "霞",
        "平", "刚", "桂英", "建华", "文", "华", "金凤", "素英", "建国", "德华"
    ]
    
    return random.choice(first_names) + random.choice(second_names)

def generate_random_email(name_en):
    """生成随机邮箱"""
    domains = ["gmail.com", "163.com", "qq.com", "sina.com", "hotmail.com", "outlook.com"]
    return f"{name_en}@{random.choice(domains)}"

def generate_random_phone():
    """生成随机手机号"""
    prefixes = ["130", "131", "132", "133", "134", "135", "136", "137", "138", "139",
                "150", "151", "152", "153", "155", "156", "157", "158", "159",
                "180", "181", "182", "183", "184", "185", "186", "187", "188", "189"]
    return random.choice(prefixes) + ''.join([str(random.randint(0, 9)) for _ in range(8)])

def generate_random_address():
    """生成随机地址"""
    provinces = ["北京市", "上海市", "广东省", "浙江省", "江苏省", "山东省", "河南省", "四川省", "湖北省", "湖南省"]
    cities = ["海淀区", "朝阳区", "浦东新区", "天河区", "西湖区", "鼓楼区", "武昌区", "锦江区"]
    streets = ["中山路", "人民路", "建设路", "解放路", "和平路", "友谊路", "胜利路", "光明路", "幸福路", "团结路"]
    
    return f"{random.choice(provinces)}{random.choice(cities)}{random.choice(streets)}{random.randint(1, 999)}号"

def generate_random_company():
    """生成随机公司名"""
    company_types = ["科技有限公司", "贸易有限公司", "投资有限公司", "咨询有限公司", "服务有限公司"]
    company_names = ["华为", "腾讯", "阿里巴巴", "百度", "京东", "美团", "滴滴", "字节跳动", "小米", "网易"]
    
    return random.choice(company_names) + random.choice(company_types)

def generate_test_data(num_records=500000, output_file="examples/data/large_test_data.csv"):
    """生成测试数据"""
    print(f"🚀 开始生成 {num_records:,} 条测试数据...")
    
    # 确保输出目录存在
    os.makedirs(os.path.dirname(output_file), exist_ok=True)
    
    # 生成数据
    with open(output_file, 'w', newline='', encoding='utf-8') as csvfile:
        fieldnames = [
            'id', 'name', 'email', 'age', 'phone', 'address', 
            'company', 'salary', 'department', 'join_date', 
            'status', 'score', 'level', 'notes'
        ]
        writer = csv.DictWriter(csvfile, fieldnames=fieldnames)
        
        # 写入标题行
        writer.writeheader()
        
        # 生成数据行
        for i in range(1, num_records + 1):
            # 生成英文名用于邮箱
            name_en = ''.join(random.choices(string.ascii_lowercase, k=random.randint(5, 10)))
            
            # 生成随机日期
            start_date = datetime(2020, 1, 1)
            end_date = datetime(2024, 12, 31)
            random_date = start_date + timedelta(
                days=random.randint(0, (end_date - start_date).days)
            )
            
            row = {
                'id': i,
                'name': generate_random_name(),
                'email': generate_random_email(name_en),
                'age': random.randint(22, 65),
                'phone': generate_random_phone(),
                'address': generate_random_address(),
                'company': generate_random_company(),
                'salary': random.randint(5000, 50000),
                'department': random.choice(['技术部', '销售部', '市场部', '人事部', '财务部', '运营部']),
                'join_date': random_date.strftime('%Y-%m-%d'),
                'status': random.choice(['active', 'inactive', 'pending']),
                'score': round(random.uniform(60.0, 100.0), 2),
                'level': random.choice(['junior', 'middle', 'senior', 'expert']),
                'notes': f"测试用户{i}的备注信息"
            }
            
            writer.writerow(row)
            
            # 每10万条记录显示进度
            if i % 100000 == 0:
                print(f"✅ 已生成 {i:,} 条记录...")
    
    # 获取文件大小
    file_size = os.path.getsize(output_file)
    print(f"🎉 数据生成完成！")
    print(f"📁 文件路径: {output_file}")
    print(f"📊 记录数量: {num_records:,} 条")
    print(f"💾 文件大小: {file_size / 1024 / 1024:.2f} MB")

if __name__ == "__main__":
    generate_test_data()
